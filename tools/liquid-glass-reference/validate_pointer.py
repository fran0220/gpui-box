"""Validate the separate synthetic NSEvent receipt experiment, not deformation.

Window receipts are checked independently of dispatches. Capture brackets are
relative to clock_origin_uptime; event/receipt/action timestamps are absolute.
Hashes establish bundle consistency, not authenticity or hardware provenance.
"""
from datetime import datetime
import json
import math
from pathlib import Path
import sys

from metrics import read_json
from validate import BACKEND, digest, png_size, ppm, require, validate

INTERACTION = 'synthetic NSEvent via NSApplication.postEvent; native hit testing and SwiftUI Button action; not hardware input'


def number(value):
    require(type(value) in (int, float) and math.isfinite(value), 'invalid timestamp')
    return value


def validate_run(run, frames, origin):
    dispatches, receipts = run['dispatches'], run['window_receipts']
    require(len(dispatches) == len(receipts) == 2, 'need independent down/up dispatches and receipts')
    for records in (dispatches, receipts):
        require([r['type'] for r in records] == ['down', 'up'], 'down/up order mismatch')
        require(all(type(r['event_number']) is int for r in records), 'invalid event identity type')
        require([r['event_number'] for r in records] == [101, 102], 'event identity mismatch')
    for dispatch, receipt in zip(dispatches, receipts):
        event = number(dispatch['event_uptime'])
        require(abs(event-number(receipt['event_uptime'])) <= 1e-6, 'receipt event timestamp mismatch')
        require(origin <= event <= number(dispatch['dispatch_uptime']) <= number(receipt['received_uptime']), 'dispatch/receipt order mismatch')
    down, up = dispatches
    dr, ur = receipts
    require(dr['received_uptime'] < up['event_uptime'], 'up preceded down receipt')
    actions = run['action_acknowledgements_uptime']
    require(len(actions) == 1 and run['expanded'] is True, 'expected exactly one expanded action')
    action = number(actions[0])
    require(ur['received_uptime'] <= action, 'action before release receipt')
    expected = [('pointer-before', 0)] + [('pointer-held', i) for i in range(3)] + [('pointer-release', i) for i in range(16)]
    require(all(type(f['index']) is int for f in frames), 'invalid pointer frame index')
    require([(f['phase'], f['index']) for f in frames] == expected, 'missing or out-of-order pointer frames')
    previous = origin
    for f in frames:
        start, end = (origin + number(f[k]) for k in ('capture_start', 'capture_end'))
        require(previous <= start < end and end-start < 1, 'invalid capture bracket')
        previous = end
        if f['phase'] == 'pointer-before':
            require(end <= down['event_uptime'], 'before frame overlaps down')
        elif f['phase'] == 'pointer-held':
            require(dr['received_uptime'] <= start and end <= up['event_uptime'], 'held frame outside down/up interval')
        else:
            require(action <= start, 'release capture precedes action acknowledgement')
    require(origin + frames[-1]['capture_start'] - action >= .8, 'missing settled release capture')
    return {'appearance': run['appearance'], 'actions_after_release': 1,
            'held_seconds': up['event_uptime'] - down['event_uptime'],
            'release_to_action_seconds': action - up['dispatch_uptime']}


def validate_pointer(root, report=None):
    root = Path(root)
    p = read_json(root / 'pointer.json') if report is None else report
    baseline = read_json(root / 'manifest.json')
    validate(root, baseline)
    require(type(p['schema']) is int and type(p['scale']) is int, 'invalid pointer schema or scale type')
    require(p['schema'] == 1 and p['capture_backend'] == BACKEND and p['interaction'] == INTERACTION, 'unsupported pointer provenance')
    require(digest(root / 'manifest.json') == p['baseline_manifest_sha256'], 'baseline manifest hash mismatch')
    require(p['provenance'] == baseline['provenance'], 'pointer provenance mismatch')
    require(p['logical_size'] == baseline['logical_size'] and p['scale'] == baseline['scale'], 'pointer dimensions mismatch')
    require(p['point_top_left'] == [136, 472] and p['point_window_bottom_left'] == [136, 168], 'pointer coordinate mismatch')
    origin = number(p['clock_origin_uptime'])
    require(origin > 0, 'invalid clock origin')
    require([r['appearance'] for r in p['runs']] == ['light', 'dark'], 'missing or duplicate pointer runs')
    require(len(p['frames']) == 40, 'expected 40 pointer frames')
    names, previous = set(), -1
    for f in p['frames']:
        require(f['appearance'] in ('light', 'dark'), 'unknown appearance')
        start, end = number(f['capture_start']), number(f['capture_end'])
        require(previous <= start < end, 'global capture ordering mismatch')
        previous = end
        require(datetime.fromisoformat(f['wall_end'].replace('Z', '+00:00')).tzinfo is not None, 'invalid wall timestamp')
        require(f['rgb_color_space'] == 'sRGB', 'unsupported sidecar color space')
        for field in ('file', 'rgb_file'):
            name = f[field]
            require(isinstance(name, str) and name not in ('', '.', '..') and '\\' not in name and Path(name).name == name and name not in names, 'unsafe or duplicate pointer path')
            names.add(name)
            require((root / name).is_file() and not (root / name).is_symlink(), 'missing or symlink pointer frame')
            require(digest(root / name) == f[field + '_sha256'], 'pointer frame hash mismatch')
        width, height = png_size((root / f['file']).read_bytes())
        require([width, height] == f['pixel_size'] == [v*p['scale'] for v in p['logical_size']], 'pointer frame dimension mismatch')
        require(ppm(root / f['rgb_file'])[:2] == (width, height), 'pointer sidecar dimension mismatch')
    results = [validate_run(run, [f for f in p['frames'] if f['appearance'] == run['appearance']], origin) for run in p['runs']]
    return {'status': 'synthetic-pointer-prerequisites-passed', 'frames': 40, 'runs': results,
            'limitations': 'Synthetic NSEvent only; no hardware input, deformation, optical equivalence, or calibration claim. Hashes prove consistency only.'}


if __name__ == '__main__':
    try:
        require(len(sys.argv) == 2, 'usage: validate_pointer.py REFERENCE')
        print(json.dumps(validate_pointer(sys.argv[1]), indent=2, allow_nan=False))
    except (ValueError, KeyError, TypeError, OSError, IndexError) as error:
        sys.exit('pointer rejected: ' + str(error))
