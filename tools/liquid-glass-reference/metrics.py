"""Descriptive sRGB-code optical evidence, never a fit or equivalence verdict.

Samples pixel centers on a one-logical-point lattice (no scale-dependent weighting).
Capsule center bands exclude labels conservatively, not via OCR. Transition uses
the union of possible centered label positions, with padding; this deliberately
throws away much of the interior. Font overflow, shadows and moving glyphs outside
that envelope remain a limitation. Rim gradients are a distortion *proxy*, not a
recovered displacement field. Transmission is background regression, not physical
transmittance; checker contrast is signed even-minus-odd RGB, excluding grid lines.
Morphology area/centroid/IoU describe only the retained nontext domain, not the
whole object. Threshold 8 is fixed, not fitted. Unequal near/far gaps and checker
alignment preclude a causal fusion claim. Timing is a capture bracket, not a
presentation timestamp. Hashes prove file consistency, not evidence authenticity.
"""
import json
import math
from pathlib import Path
import sys

from compare import pair
from validate import digest, ppm, require, validate


def read_json(path):
    def unique(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate JSON key: " + key)
            result[key] = value
        return result
    def invalid(value):
        raise ValueError("nonfinite JSON number: " + value)
    return json.loads(Path(path).read_text(), object_pairs_hook=unique, parse_constant=invalid)


def capsule_distance(x, y, rect):
    left, top, width, height = rect
    radius = height / 2
    center = min(max(x, left + radius), left + width - radius)
    return math.hypot(x - center, y - top - radius) - radius


def pill_masks(rect):
    left, top, width, height = rect
    interior, rim = [], []
    for y in range(int(top - 4), int(top + height + 4)):
        for x in range(int(left - 4), int(left + width + 4)):
            # Entire horizontal band, including curved ends: no guessed glyph widths.
            if abs(y + .5 - top - height / 2) <= 13:
                continue
            distance = capsule_distance(x + .5, y + .5, rect)
            if distance < -6:
                interior.append((x, y))
            if -4 <= distance <= 4:
                rim.append((x, y))
    require(interior and rim, "empty capsule masks")
    return interior, rim


def transition_mask(fixture):
    button, menu = fixture['button'], fixture['menu']
    x, y, w, h = menu
    # Centered Actions and centered VStack text sweep between these centers.
    lo = min(button[0] + button[2]/2, x + w/2) - 56
    hi = max(button[0] + button[2]/2, x + w/2) + 56
    return [(xx, yy) for yy in range(y - 8, y + h + 8)
            for xx in range(x - 8, x + w + 8)
            if not (lo <= xx + .5 <= hi and y + 4 <= yy + .5 <= y + h - 4)]


def bridge_mask(panes):
    a, b = panes
    require(a[1] == b[1] and a[3] == b[3] and b[0] > a[0]+a[2], 'invalid fusion geometry')
    return [(x, y) for y in range(a[1]+8, a[1]+a[3]-8)
            for x in range(a[0]+a[2]+2, b[0]-2)]


def sample(image, point, scale):
    width, height, data = image
    x, y = (int((v + .5) * scale) for v in point)
    require(0 <= x < width and 0 <= y < height, "mask outside image")
    offset = (y * width + x) * 3
    return tuple(data[offset:offset + 3])


def mean(values):
    require(bool(values), "empty measurement")
    return sum(values) / len(values)


def region(reference, candidate, ref_background, cand_background, points, scale, cell):
    require(points and len(points) == len(set(points)), "empty or duplicate mask")
    r, c, rb, cb = [[sample(image, p, scale) for p in points]
                    for image in (reference, candidate, ref_background, cand_background)]
    errors = [[c[i][k] - r[i][k] for i in range(len(points))] for k in range(3)]
    def optics(values, background):
        slopes, contrast, background_contrast = [], [], []
        groups = [[i for i, (x, y) in enumerate(points)
                   if (x // cell + y // cell) % 2 == parity
                   and 2 <= x % cell < cell - 2 and 2 <= y % cell < cell - 2]
                  for parity in (0, 1)]
        for k in range(3):
            b = [p[k] for p in background]
            v = [p[k] for p in values]
            mb, mv = mean(b), mean(v)
            variance = sum((a - mb)**2 for a in b)
            slopes.append(sum((a-mb)*(z-mv) for a, z in zip(b, v))/variance if variance else None)
            contrast.append(mean([v[i] for i in groups[0]]) - mean([v[i] for i in groups[1]]) if all(groups) else None)
            background_contrast.append(mean([b[i] for i in groups[0]]) - mean([b[i] for i in groups[1]]) if all(groups) else None)
        return {'background_regression_rgb': slopes, 'checker_contrast_rgb': contrast,
                'background_checker_contrast_rgb': background_contrast,
                'background_residual_mae': mean([abs(a-b) for v, bg in zip(values, background) for a, b in zip(v, bg)])}
    # Forward differences only where BOTH endpoints belong to the nontext mask.
    lookup = {p: i for i, p in enumerate(points)}
    gradients, ref_distortion, cand_distortion = [], [], []
    for i, (x, y) in enumerate(points):
        for q in ((x+1, y), (x, y+1)):
            if q not in lookup:
                continue
            j = lookup[q]
            for k in range(3):
                rg, cg = r[j][k]-r[i][k], c[j][k]-c[i][k]
                gradients.append(abs(cg-rg))
                ref_distortion.append(abs(rg-(rb[j][k]-rb[i][k])))
                cand_distortion.append(abs(cg-(cb[j][k]-cb[i][k])))
    return {'samples': len(points), 'rgb_mae': [mean([abs(v) for v in e]) for e in errors],
            'mae': mean([abs(v) for e in errors for v in e]),
            'rgb_bias': [mean(e) for e in errors],
            'rmse': math.sqrt(mean([v*v for e in errors for v in e])),
            'gradient_mae': mean(gradients) if gradients else None,
            'reference_rim_gradient_residual': mean(ref_distortion) if gradients else None,
            'candidate_rim_gradient_residual': mean(cand_distortion) if gradients else None,
            'reference': optics(r, rb), 'candidate': optics(c, cb)}


def morphology(reference, candidate, rb, cb, points, scale, threshold=8):
    def occupied(image, background):
        return {p for p in points if mean([abs(a-b) for a, b in
                zip(sample(image, p, scale), sample(background, p, scale))]) > threshold}
    r, c = occupied(reference, rb), occupied(candidate, cb)
    def shape(mask):
        if not mask:
            return {'area': 0, 'centroid': None, 'bounds': None}
        xs, ys = zip(*mask)
        return {'area': len(mask), 'centroid': [mean(xs)+.5, mean(ys)+.5],
                'bounds': [min(xs), min(ys), max(xs)+1, max(ys)+1]}
    return {'threshold_rgb_mae': threshold, 'evaluated_samples': len(points),
            'reference': shape(r), 'candidate': shape(c),
            'iou': len(r & c)/len(r | c) if r | c else 1,
            'symmetric_difference': len(r ^ c), 'area_error': len(c)-len(r)}


def analyze(ref_dir, candidate_dir):
    ref_dir, candidate_dir = Path(ref_dir), Path(candidate_dir)
    reference = read_json(ref_dir / 'manifest.json')
    validate(ref_dir, reference)
    candidate = read_json(candidate_dir / 'candidate.json')
    for manifest in (reference, candidate):
        require(type(manifest['schema']) is int and type(manifest['scale']) is int, 'invalid schema or scale type')
        for frame in manifest['frames']:
            require(type(frame['index']) is int, 'invalid frame index')
            if 'pixel_size' in frame:
                require(frame['pixel_size'] == [v*manifest['scale'] for v in manifest['logical_size']], 'reported frame dimension mismatch')
            for key in ('capture_start', 'capture_end', 'trigger_time', 'sample_time_after_trigger'):
                if key in frame:
                    require(type(frame[key]) in (int, float) and math.isfinite(frame[key]), 'invalid timestamp')
    pairing = pair(reference, candidate, candidate_dir)
    fixture = read_json(ref_dir / 'fixture.json')
    require(fixture == read_json(Path(__file__).with_name('fixture.json')), 'unsupported fixture geometry or identities')
    require(digest(candidate_dir / 'parameters.json') == candidate['parameters_sha256'], 'parameter hash mismatch')
    frames = {(f['appearance'], f['phase'], f['index']): f for f in reference['frames']}
    pairs = {p['reference']: p['candidate'] for p in pairing['pairs']}
    scale, cell = reference['scale'], fixture['background']['cell']
    def images(key):
        name = frames[key]['rgb_file']
        return ppm(ref_dir / name), ppm(candidate_dir / pairs[name])
    report = {'status': 'descriptive-evidence-only', 'pairs': len(pairs),
              'units': 'sRGB code values 0..255; geometry in logical points',
              'hash_scope': 'native manifest prerequisites (PNG, PPM, source, fixture); candidate paired PPM and parameters.json. Candidate optional PNG/raw files are not consumed or validated.',
              'reference_manifest_sha256': digest(ref_dir / 'manifest.json'),
              'candidate_manifest_sha256': digest(candidate_dir / 'candidate.json'),
              'parameters_sha256': candidate['parameters_sha256'],
              'mask_policy': 'capsule signed distance interior < -6, rim [-4,+4]; exclude full center 26-point label band; bridges inset 2 horizontally and 8 vertically; transition glyph swept centers +/-56, y inset 4; one sample per logical point',
              'split_policy': 'training: regular-large/light/static only; held-out: regular-small/light and all dark; other light regions diagnostic only. No fitting performed.',
              'limitations': __doc__, 'pills': [], 'fusion': [], 'transition': []}
    for appearance in ('light', 'dark'):
        rb, cb = images((appearance, 'background', 0))
        r, c = images((appearance, 'static', 0))
        for pill in fixture['pills']:
            interior, rim = pill_masks(pill['rect'])
            split = ('held-out' if appearance == 'dark' or pill['id'] == 'regular-small'
                     else 'training' if pill['id'] == 'regular-large' else 'diagnostic')
            report['pills'].append({'id': pill['id'], 'appearance': appearance, 'split': split,
                'interior': region(r, c, rb, cb, interior, scale, cell),
                'rim': region(r, c, rb, cb, rim, scale, cell)})
        fusion = {'appearance': appearance, 'split': 'held-out' if appearance == 'dark' else 'diagnostic'}
        for mode in ('near', 'far'):
            points = bridge_mask(fixture['fusion'][mode])
            fusion[mode] = region(r, c, rb, cb, points, scale, cell)
        for side in ('reference', 'candidate'):
            fusion[side + '_near_minus_far_residual'] = (fusion['near'][side]['background_residual_mae'] - fusion['far'][side]['background_residual_mae'])
        fusion['near_minus_far_error'] = fusion['candidate_near_minus_far_residual'] - fusion['reference_near_minus_far_residual']
        report['fusion'].append(fusion)
        points = transition_mask(fixture['transition'])
        for index in range(fixture['transition']['samples']):
            r, c = images((appearance, 'transition', index))
            f = frames[appearance, 'transition', index]
            report['transition'].append({'appearance': appearance, 'index': index,
                'split': 'held-out' if appearance == 'dark' else 'diagnostic',
                'time_bracket': [f[k]-f['trigger_time'] for k in ('capture_start', 'capture_end')],
                'morphology': morphology(r, c, rb, cb, points, scale)})
    return report


if __name__ == '__main__':
    try:
        require(len(sys.argv) == 3, 'usage: metrics.py REFERENCE CANDIDATE')
        print(json.dumps(analyze(*sys.argv[1:]), indent=2, allow_nan=False))
    except (ValueError, KeyError, TypeError, OSError, IndexError) as error:
        sys.exit('metrics rejected: ' + str(error))
