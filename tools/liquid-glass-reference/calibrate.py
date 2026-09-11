"""Enumerate explicit static GPUI trials against validated native evidence.

CLI: calibrate.py REFERENCE OUTPUT --candidates LIST.json --appearance light|dark
     --executable PATH --revision ID

LIST.json is 1..32 distinct, nonempty objects of bounded regular_* setters from
gpui_capture.LIMITS, optionally protect_text=0 (always forced to 0). Omitted
setters retain the hashed production defaults. No adaptive search is performed.
OUTPUT and every candidate directory must be fresh; failures are retained.

Predeclared training: regular-large, static, in --appearance ONLY. Objective:
interior MAE + 0.25 * rim MAE in sRGB code values. This is a diagnostic objective,
not perceptual/physical truth or an Apple-equivalence threshold. Ties select the
first supplied candidate. Regular-small in that appearance is held out; other
pills and fusion bridges are diagnostics. The other appearance measures transfer,
not tuned output. Install a chosen profile ONLY in the selected theme: regular
parameters are currently global within each capture. No production edits occur.

Only four static/control smoke frames per candidate are rendered. Validation of
the 36-frame native reference does not turn this into 36-frame calibration or
dynamics evidence. Conservative nontext masks are analytic, not glyph/optical
segmentation. Fusion pane contours, outsets and transitions are not scored.
Clear invariance is measured against candidate zero, NOT against Apple.
Visual review remains required. Hashes establish consistency, not authenticity.
"""
import argparse
from pathlib import Path
import sys

import gpui_capture
from metrics import bridge_mask, pill_masks, region
from validate import digest, png_size, ppm, require, validate

HERE = Path(__file__).resolve().parent
KEYS = {(a, p, 0) for a in ('light', 'dark') for p in ('background', 'static')}


def candidates(value):
    require(isinstance(value, list) and 1 <= len(value) <= 32,
            'supply 1..32 explicit candidates')
    result, seen = [], set()
    allowed = {k for k in gpui_capture.LIMITS if k.startswith('regular_')} | {'protect_text'}
    for item in value:
        require(isinstance(item, dict) and bool(item) and set(item) <= allowed,
                'candidate must be a nonempty regular_* parameter object')
        gpui_capture.parameters(item)
        require(item.get('protect_text', 0) == 0, 'calibration requires protect_text=0')
        options = {**item, 'protect_text': 0}
        identity = tuple(sorted(options.items()))
        require(identity not in seen, 'duplicate effective candidate')
        seen.add(identity)
        result.append(options)
    return result


def checked_file(root, name, expected_hash, seen):
    path = gpui_capture.output_file(root, name, seen)
    require(digest(path) == expected_hash, 'candidate hash mismatch: ' + name)
    return path


def validate_smoke(root, options, scale, executable, revision):
    """Validate persisted producer output, not the capture function's return value."""
    root = Path(root)
    m = gpui_capture.load(root / 'smoke.json')
    require(type(m['schema']) is int and m['schema'] == 1 and m['producer'] == 'gpui',
            'unsupported candidate schema/producer')
    require(m['status'] == 'smoke-only; no native reference validated', 'not static smoke evidence')
    require(m['revision'] == revision and type(m['scale']) is int and m['scale'] == scale
            and m['logical_size'] == [960, 640], 'candidate identity/scale mismatch')
    require(m['fixture_sha256'] == digest(HERE / 'fixture.json'), 'candidate fixture mismatch')
    p, names = m['provenance'], {'smoke.json'}
    require(p['reference_manifest_sha256'] is None, 'smoke unexpectedly claims native pairing')
    require(p['executable_sha256'] == digest(executable), 'executable identity mismatch')
    require(p['command'] == [str(Path(executable).resolve()), str(root / 'request.json'), str(root)],
            'candidate command mismatch')
    params = checked_file(root, 'parameters.json', m['parameters_sha256'], names)
    require(gpui_capture.load(params) == options, 'candidate parameters mismatch')
    request = gpui_capture.load(checked_file(root, 'request.json', p['request_sha256'], names))
    expected = gpui_capture.plan({'logical_size': [960, 640], 'scale': scale,
        'frames': [{'appearance': a, 'phase': phase, 'index': 0}
                   for a in ('light', 'dark') for phase in ('background', 'static')]},
        gpui_capture.load(HERE / 'fixture.json'), options)
    require(request == expected, 'candidate request mismatch')
    report = gpui_capture.load(checked_file(root, 'render.json', p['render_report_sha256'], names))
    require(type(report['schema']) is int and report['schema'] == 1
            and report['renderer'] == m['renderer']
            and m['renderer'] in ('native-metal', 'wgpu-software-fallback'), 'unknown renderer')
    require(report['clock'] == 'GPUI TestDispatcher; measured executor now at draw completion',
            'unknown clock')
    require(isinstance(p['source_sha256'], dict) and p['source_sha256'], 'missing source identities')
    for name, sha in p['source_sha256'].items():
        path = (gpui_capture.REPO / name).resolve()
        require(not Path(name).is_absolute() and path.is_relative_to(gpui_capture.REPO),
                'unsafe source path')
        require(digest(path) == sha, 'source identity mismatch: ' + name)
    returned = {}
    for f in report['frames']:
        k = gpui_capture.key(f)
        require(k in KEYS and k not in returned, 'unexpected or duplicate renderer frame')
        returned[k] = f
    require(set(returned) == KEYS, 'missing renderer frames')
    images = {}
    size = (960 * scale, 640 * scale)
    for f in m['frames']:
        k = gpui_capture.key(f)
        require(k in KEYS and k not in images, 'unexpected or duplicate candidate frame')
        rf = returned[k]
        for field in ('appearance', 'phase', 'index', 'pixel_size', 'sample_time_after_trigger',
                      'transition_status', 'file', 'raw_file'):
            require(f[field] == rf[field], 'renderer/manifest disagreement: ' + field)
        require(f['pixel_size'] == list(size), 'candidate dimensions mismatch')
        require(gpui_capture.number(f['sample_time_after_trigger'])
                and f['sample_time_after_trigger'] == 0
                and f['transition_status'] == 'not-applicable', 'nonstatic candidate')
        paths = {field: checked_file(root, f[field], f[field + '_sha256'], names)
                 for field in ('file', 'raw_file', 'rgb_file')}
        require(png_size(paths['file'].read_bytes()) == size, 'PNG dimensions mismatch')
        image = ppm(paths['rgb_file'])
        require(image[:2] == size, 'PPM dimensions mismatch')
        require(paths['raw_file'].read_bytes() == image[2], 'raw/PPM mismatch')
        images[k] = image
    require(set(images) == KEYS, 'missing candidate frames')
    return m, images


def split(appearance, selected, pill_id):
    if appearance != selected:
        return 'cross-appearance-transfer'
    if pill_id == 'regular-large':
        return 'training'
    if pill_id == 'regular-small':
        return 'held-out'
    return 'diagnostic'


def measure(reference, candidate, first, fixture, scale, appearance):
    pills, fusion, clear = [], [], []
    cell = fixture['background']['cell']
    for a in ('light', 'dark'):
        r, c, rb, cb = (images[(a, phase, 0)] for images, phase in
                        ((reference, 'static'), (candidate, 'static'),
                         (reference, 'background'), (candidate, 'background')))
        for pill in fixture['pills']:
            masks = dict(zip(('interior', 'rim'), pill_masks(pill['rect'])))
            pills.append({'id': pill['id'], 'appearance': a,
                'split': split(a, appearance, pill['id']),
                **{name: region(r, c, rb, cb, points, scale, cell)
                   for name, points in masks.items()}})
            if pill['material'] == 'clear':
                clear.append({'id': pill['id'], 'appearance': a,
                    **{name: region(first[(a, 'static', 0)], c,
                                   first[(a, 'background', 0)], cb, points, scale, cell)
                       for name, points in masks.items()}})
        for mode in ('near', 'far'):
            fusion.append({'id': mode, 'appearance': a,
                'split': 'diagnostic' if a == appearance else 'cross-appearance-transfer',
                'bridge': region(r, c, rb, cb, bridge_mask(fixture['fusion'][mode]), scale, cell)})
    training = [p for p in pills if p['split'] == 'training']
    require(len(training) == 1, 'expected exactly one training region')
    score = training[0]['interior']['mae'] + 0.25 * training[0]['rim']['mae']
    return {'objective': score, 'pills': pills, 'fusion': fusion,
            'clear_vs_first_candidate': clear}


def run(reference_dir, output, candidate_file, appearance, executable, revision):
    require(appearance in ('light', 'dark'), 'explicit light or dark appearance required')
    options = candidates(gpui_capture.load(candidate_file))
    require(isinstance(revision, str) and revision.strip() not in ('', 'unknown'), 'explicit revision required')
    reference_dir, output, executable = (Path(p).resolve() for p in (reference_dir, output, executable))
    require(executable.is_file(), 'missing executable')
    manifest = gpui_capture.load(reference_dir / 'manifest.json')
    validation = validate(reference_dir, manifest)  # Never substitute synthetic authentication.
    require(type(manifest['scale']) is int, 'invalid reference scale')
    fixture = gpui_capture.load(reference_dir / 'fixture.json')
    require(digest(reference_dir / 'fixture.json') == digest(HERE / 'fixture.json'), 'fixture mismatch')
    reference = {gpui_capture.key(f): ppm(reference_dir / f['rgb_file'])
                 for f in manifest['frames'] if f['phase'] in ('static', 'background')}
    require(set(reference) == KEYS, 'missing static reference frames')
    identity = {'reference_directory': str(reference_dir),
        'reference_manifest_sha256': digest(reference_dir / 'manifest.json'),
        'reference_manifest': manifest, 'reference_validation': validation,
        'executable': str(executable), 'executable_sha256': digest(executable),
        'revision': revision, 'candidate_list_sha256': digest(candidate_file),
        'driver_source_sha256': {p.name: digest(p) for p in
            (HERE / 'calibrate.py', HERE / 'gpui_capture.py', HERE / 'metrics.py', HERE / 'validate.py')}}
    output.mkdir(parents=True, exist_ok=False)
    declaration = {'schema': 1, 'appearance': appearance, 'effective_candidates': options,
        'training': {'appearance': appearance, 'phase': 'static', 'id': 'regular-large'},
        'objective': 'interior.mae + 0.25 * rim.mae; sRGB code values; diagnostic only',
        'tie_break': 'first supplied candidate', 'limitations': __doc__, 'identity': identity}
    gpui_capture.write(output / 'plan.json', declaration)  # Written before any rendering.
    results, first, source_identity = [], None, None
    try:
        for index, trial in enumerate(options):
            directory = output / f'candidate-{index:03d}'
            gpui_capture.capture(reference_dir=None, output=directory, executable=executable,
                                 revision=revision, options=trial, smoke_scale=manifest['scale'])
            m, images = validate_smoke(directory, trial, manifest['scale'], executable, revision)
            require(m['provenance']['executable_sha256'] == identity['executable_sha256'],
                    'executable changed during trials')
            current = (m['renderer'], m['provenance']['source_sha256'])
            if source_identity is None:
                source_identity = current
            require(current == source_identity, 'renderer or source changed between trials')
            if first is None:
                first = images
            result = {'index': index, 'directory': directory.name, 'parameters': trial,
                'smoke_manifest_sha256': digest(directory / 'smoke.json'), 'smoke_manifest': m,
                **measure(reference, images, first, fixture, manifest['scale'], appearance)}
            gpui_capture.write(directory / 'metrics.json', result)
            results.append(result)
        require(digest(reference_dir / 'manifest.json') == identity['reference_manifest_sha256'],
                'reference manifest changed during trials')
        validate(reference_dir, manifest)
        for name, sha in identity['driver_source_sha256'].items():
            require(digest(HERE / name) == sha, 'driver changed during trials')
        chosen = min(results, key=lambda r: r['objective'])
        report = {**declaration, 'status': 'static-diagnostic-selection; visual review required',
            'candidates': results, 'chosen_index': chosen['index'],
            'chosen_parameters': chosen['parameters'],
            'installation_scope': 'studio-' + appearance + ' ONLY; no production edits performed'}
        gpui_capture.write(output / 'calibration.json', report)
        return report
    except Exception as error:
        gpui_capture.write(output / 'failure.json', {'status': 'failed; directory retained; do not reuse',
                           'completed_candidates': len(results), 'error': str(error)})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('reference', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--candidates', required=True, type=Path)
    parser.add_argument('--appearance', required=True, choices=('light', 'dark'))
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--revision', required=True)
    args = parser.parse_args()
    try:
        run(args.reference, args.output, args.candidates, args.appearance, args.executable, args.revision)
    except (ValueError, KeyError, TypeError, OSError, IndexError) as error:
        sys.exit('calibration rejected: ' + str(error))


if __name__ == '__main__':
    main()
