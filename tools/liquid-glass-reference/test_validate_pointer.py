"""Synthetic receipt and file-corruption tests, not hardware evidence."""
import copy
import json
import os
import unittest

import test_validate
from validate import BACKEND, digest
from validate_pointer import INTERACTION, validate_pointer, validate_run


def synthetic_run(appearance='light', offset=0):
    origin = 1000
    run = {'appearance': appearance, 'expanded': True,
           'action_acknowledgements_uptime': [origin+offset+2.03],
           'dispatches': [], 'window_receipts': []}
    for kind, number, time in [('down', 101, 1), ('up', 102, 2)]:
        run['dispatches'].append({'type': kind, 'event_number': number, 'event_uptime': origin+offset+time, 'dispatch_uptime': origin+offset+time+.01})
        run['window_receipts'].append({'type': kind, 'event_number': number, 'event_uptime': origin+offset+time, 'received_uptime': origin+offset+time+.02})
    frames = []
    for phase, times in [('pointer-before', [.5]), ('pointer-held', [1.2, 1.4, 1.6]), ('pointer-release', [2.1+i*.1 for i in range(16)])]:
        for i, time in enumerate(times):
            frames.append({'appearance': appearance, 'phase': phase, 'index': i,
                           'capture_start': offset+time, 'capture_end': offset+time+.01})
    return run, frames, origin


class OrderingTests(unittest.TestCase):
    def test_complete(self):
        run, frames, origin = synthetic_run()
        self.assertEqual(validate_run(run, frames, origin)['actions_after_release'], 1)

    def test_malformed_independent_receipts_and_actions(self):
        mutations = [lambda r: r['window_receipts'].clear(),
                     lambda r: r['window_receipts'].reverse(),
                     lambda r: r['window_receipts'][1].update(event_number=101),
                     lambda r: r['window_receipts'][1].update(event_uptime=1002.1),
                     lambda r: r['window_receipts'][0].update(received_uptime=1000.9),
                     lambda r: r['window_receipts'][0].update(received_uptime=1002.02),
                     lambda r: r['action_acknowledgements_uptime'].append(1002.04),
                     lambda r: r.update(action_acknowledgements_uptime=[]),
                     lambda r: r.update(action_acknowledgements_uptime=[1002.015]),
                     lambda r: r.update(action_acknowledgements_uptime=[float('nan')]),
                     lambda r: r.update(expanded=False),
                     lambda r: r['dispatches'][0].update(event_uptime=True)]
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                run, frames, origin = synthetic_run()
                mutate(run)
                with self.assertRaises(ValueError):
                    validate_run(run, frames, origin)

    def test_frame_phase_clock_and_order(self):
        for mutate in (lambda f: f.pop(), lambda f: f.reverse(),
                       lambda f: f[0].update(capture_end=1.001),
                       lambda f: f[1].update(capture_start=.99),
                       lambda f: f[3].update(capture_end=2.001),
                       lambda f: f[4].update(capture_start=2.025),
                       lambda f: f[4].update(capture_start=float('inf'))):
            with self.subTest(mutate=mutate):
                run, frames, origin = synthetic_run()
                mutate(frames)
                with self.assertRaises(ValueError):
                    validate_run(run, frames, origin)


class PointerBundleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        test_validate.EvidenceTests.setUpClass()
        cls.root = test_validate.EvidenceTests.root
        baseline = test_validate.EvidenceTests.original
        (cls.root / 'manifest.json').write_text(json.dumps(baseline))
        cls.report = {'schema': 1, 'capture_backend': BACKEND, 'interaction': INTERACTION,
                      'provenance': baseline['provenance'], 'logical_size': [960, 640], 'scale': 1,
                      'point_top_left': [136, 472], 'point_window_bottom_left': [136, 168],
                      'baseline_manifest_sha256': digest(cls.root / 'manifest.json'),
                      'clock_origin_uptime': 1000, 'runs': [], 'frames': []}
        for appearance, offset in [('light', 0), ('dark', 5)]:
            run, frames, _ = synthetic_run(appearance, offset)
            cls.report['runs'].append(run)
            for frame in frames:
                frame.update(pixel_size=[960, 640], rgb_color_space='sRGB', wall_end='2026-09-11T00:00:00Z')
                for field, ext in [('file', 'png'), ('rgb_file', 'ppm')]:
                    name = f"{appearance}-{frame['phase']}-{frame['index']}.{ext}"
                    os.link(cls.root / f'{appearance}-static-0.{ext}', cls.root / name)
                    frame[field] = name
                    frame[field + '_sha256'] = digest(cls.root / name)
                cls.report['frames'].append(frame)

    @classmethod
    def tearDownClass(cls):
        test_validate.EvidenceTests.tearDownClass()

    def test_hash_validated_complete(self):
        self.assertEqual(validate_pointer(self.root, self.report)['frames'], 40)

    def test_malformed_files_reports(self):
        for mutate in (lambda p: p['frames'][0].update(file_sha256='0'*64),
                       lambda p: p['frames'][0].update(rgb_file_sha256='0'*64),
                       lambda p: p['frames'][0].update(rgb_file='missing.ppm'),
                       lambda p: p['frames'][0].update(pixel_size=[640, 960]),
                       lambda p: p['frames'][0].update(file='../bad.png'),
                       lambda p: p['frames'][0].update(wall_end='yesterday'),
                       lambda p: p['frames'].pop(),
                       lambda p: p.update(baseline_manifest_sha256='0'*64),
                       lambda p: p.update(clock_origin_uptime=999),
                       lambda p: p['runs'][0]['window_receipts'].clear()):
            with self.subTest(mutate=mutate):
                report = copy.deepcopy(self.report)
                mutate(report)
                with self.assertRaises((ValueError, OSError)):
                    validate_pointer(self.root, report)

    def test_hash_consistent_truncated_sidecar(self):
        path = self.root / 'truncated.ppm'
        path.write_bytes(b'P6\n960 640\n255\nabc')
        report = copy.deepcopy(self.report)
        report['frames'][0].update(rgb_file=path.name, rgb_file_sha256=digest(path))
        try:
            with self.assertRaisesRegex(ValueError, 'truncated'):
                validate_pointer(self.root, report)
        finally:
            path.unlink()


if __name__ == '__main__':
    unittest.main()
