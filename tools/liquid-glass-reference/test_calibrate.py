"""Static calibration tests; synthetic pixels are never native authentication.

Only subprocess.run is mocked. The integration test uses the real run-005 native
receipt when present, otherwise explicitly skips; it never fabricates a receipt.
"""
import copy
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import zlib

import calibrate as c
import gpui_capture as g


def png(width, height, rgb):
    def chunk(kind, value):
        return (struct.pack('>I', len(value)) + kind + value
                + struct.pack('>I', zlib.crc32(kind + value)))
    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress((b'\0' + bytes(rgb) * width) * height))
            + chunk(b'IEND', b''))


def render(command, **kwargs):
    """Synthetic process-boundary output, not a native fixture or renderer claim."""
    request, root = g.load(command[1]), Path(command[2])
    width, height = (v * request['scale'] for v in (960, 640))
    frames = []
    for i, frame in enumerate(request['frames']):
        rgb = (19, 73, 151) if frame['phase'] == 'background' else (25, 99, 219)
        (root / f'{i}.rgb').write_bytes(bytes(rgb) * (width * height))
        (root / f'{i}.png').write_bytes(png(width, height, rgb))
        frames.append({**frame, 'raw_file': f'{i}.rgb', 'file': f'{i}.png',
                       'pixel_size': [width, height], 'sample_time_after_trigger': 0,
                       'transition_status': 'not-applicable'})
    g.write(root / 'render.json', {'schema': 1, 'renderer': 'wgpu-software-fallback',
        'clock': 'GPUI TestDispatcher; measured executor now at draw completion', 'frames': frames})


class Calibration(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.exe = self.root / 'executable'
        self.exe.write_bytes(b'only a test process-boundary identity; never executed')
        self.options = {'regular_gain': 1.2, 'protect_text': 0}

    def smoke(self, name='capture', scale=1):
        output = self.root / name
        with patch.object(g.subprocess, 'run', side_effect=render):
            g.capture(None, output, self.exe, 'test-revision', self.options, scale)
        return output

    def check(self, output, scale=1):
        return c.validate_smoke(output, self.options, scale, self.exe, 'test-revision')

    def test_explicit_candidates_and_parameter_authority(self):
        self.assertEqual(c.candidates([{'regular_gain': 1.2}]), [self.options])
        self.assertEqual(len(c.candidates([{'regular_blur': i} for i in range(32)])), 32)
        bad = [[], {}, [None], [{}], [1], [{'blur': 1}], [{'protect_text': 1}],
               [{'regular_gain': 1}, {'regular_gain': 1.0, 'protect_text': 0}],
               [{'regular_blur': i} for i in range(33)]]
        for key, (lo, hi) in g.LIMITS.items():
            if not key.startswith('regular_'):
                continue
            for value in (lo, hi):
                self.assertEqual(c.candidates([{key: value}]), [{key: value, 'protect_text': 0}])
            bad.extend([{key: v}] for v in (lo - .01, hi + .01, True, '1', None,
                                          float('nan'), float('inf'), [], {}))
        for value in bad:
            with self.subTest(value=value), self.assertRaises(ValueError):
                c.candidates(value)

    def test_bad_json(self):
        path = self.root / 'list.json'
        for text in ('[{"regular_gain":1,"regular_gain":2}]', '[{"regular_gain":NaN}]',
                     '[{"regular_gain":1e999}]', '[', '[{"regular_gain":1},]'):
            path.write_text(text)
            with self.subTest(text=text), self.assertRaises(ValueError):
                c.candidates(g.load(path))

    def test_asymmetric_scores_and_no_held_out_leakage(self):
        fixture = g.load(g.HERE / 'fixture.json')
        base = bytes((20, 70, 150)) * (960 * 640)
        reference = {k: (960, 640, base) for k in c.KEYS}
        candidate = dict(reference)
        pixels = bytearray(base)
        # Independently specified channel deltas: interior (+3,-6,+12),
        # rim (-9,+15,-24). MAEs 7 and 16 => objective 11, not pooled MAE.
        interior, rim = c.pill_masks([64, 160, 208, 72])
        for points, rgb in ((interior, (23, 64, 162)), (rim, (11, 85, 126))):
            for x, y in points:
                offset = (y * 960 + x) * 3
                pixels[offset:offset + 3] = bytes(rgb)
        # Held-out small: enormous error. All other appearance pixels also
        # differ asymmetrically. Neither is allowed to move the light objective.
        for y in range(70, 125):
            for x in range(55, 215):
                offset = (y * 960 + x) * 3
                pixels[offset:offset + 3] = bytes((200, 220, 240))
        candidate['light', 'static', 0] = (960, 640, bytes(pixels))
        candidate['dark', 'static', 0] = (960, 640, bytes((26, 58, 174)) * (960 * 640))
        light = c.measure(reference, candidate, candidate, fixture, 1, 'light')
        self.assertEqual(light['objective'], 11)
        training = next(p for p in light['pills'] if p['split'] == 'training')
        self.assertEqual(training['interior']['rgb_mae'], [3, 6, 12])
        self.assertEqual(training['interior']['rgb_bias'], [3, -6, 12])
        self.assertEqual(training['rim']['rgb_mae'], [9, 15, 24])
        held = next(p for p in light['pills'] if p['split'] == 'held-out')
        self.assertEqual(held['id'], 'regular-small')
        self.assertEqual(held['interior']['mae'], 140)
        self.assertTrue(all(p['split'] == 'cross-appearance-transfer'
                            for p in light['pills'] if p['appearance'] == 'dark'))
        self.assertTrue(all(p['interior']['mae'] == p['rim']['mae'] == 0
                            for p in light['clear_vs_first_candidate']))
        dark = c.measure(reference, candidate, reference, fixture, 1, 'dark')
        self.assertEqual(dark['objective'], 17.5)  # (6+12+24)/3 * 1.25
        clear = next(p for p in dark['clear_vs_first_candidate'] if p['appearance'] == 'dark')
        self.assertEqual(clear['interior']['mae'], 14)  # candidate-to-first, not Apple
        self.assertEqual(len(light['fusion']), 4)

    def test_smoke_roundtrip_and_scale(self):
        for scale in (1, 2):
            output = self.smoke(str(scale), scale)
            m, images = self.check(output, scale)
            self.assertEqual(set(images), c.KEYS)
            self.assertEqual(images['dark', 'static', 0][:2], (960 * scale, 640 * scale))
            self.assertEqual(images['light', 'static', 0][2][:3], bytes((25, 99, 219)))
            self.assertIsNone(m['provenance']['reference_manifest_sha256'])

    def test_malformed_manifests(self):
        output = self.smoke()
        original = g.load(output / 'smoke.json')
        changes = [lambda m: m.update(schema=True), lambda m: m.update(scale=True),
            lambda m: m.update(revision='wrong'), lambda m: m.update(frames=[]),
            lambda m: m['frames'].append(m['frames'][0]),
            lambda m: m['frames'][0].update(index=True),
            lambda m: m['frames'][0].update(rgb_file='../escape.ppm'),
            lambda m: m['frames'][0].update(rgb_file=m['frames'][1]['rgb_file']),
            lambda m: m['frames'][0].update(pixel_size=[640, 960]),
            lambda m: m['frames'][0].update(sample_time_after_trigger=True),
            lambda m: m['frames'][0].update(rgb_file_sha256='0' * 64),
            lambda m: m['provenance'].update(executable_sha256='0' * 64),
            lambda m: m['provenance'].update(source_sha256={}),
            lambda m: m['provenance'].update(source_sha256={'../escape': '0' * 64}),
            lambda m: m['provenance'].update(reference_manifest_sha256='0' * 64)]
        for change in changes:
            m = copy.deepcopy(original)
            change(m)
            g.write(output / 'smoke.json', m)
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.check(output)

    def test_malformed_renderer_reports_with_updated_hash(self):
        output = self.smoke()
        manifest = g.load(output / 'smoke.json')
        report = g.load(output / 'render.json')
        for change in (lambda r: r['frames'].pop(), lambda r: r['frames'].append(r['frames'][0]),
                       lambda r: r.update(clock='fake'), lambda r: r.update(schema=True),
                       lambda r: r['frames'][0].update(sample_time_after_trigger=1),
                       lambda r: r['frames'][0].update(file='another.png')):
            r = copy.deepcopy(report)
            change(r)
            g.write(output / 'render.json', r)
            manifest['provenance']['render_report_sha256'] = g.digest(output / 'render.json')
            g.write(output / 'smoke.json', manifest)
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.check(output)

    def test_files_are_hashed_and_dimensions_checked(self):
        output = self.smoke()
        original = g.load(output / 'smoke.json')
        f = original['frames'][0]
        for field, payload in (('file', png(640, 960, (19, 73, 151))),
                               ('rgb_file', b'P6\n640 960\n255\n' + bytes((19, 73, 151)) * (960 * 640)),
                               ('raw_file', b'short')):
            path = output / f[field]
            saved = path.read_bytes()
            path.write_bytes(payload)
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                self.check(output)
            m = copy.deepcopy(original)
            m['frames'][0][field + '_sha256'] = g.digest(path)
            g.write(output / 'smoke.json', m)
            with self.assertRaises(ValueError):
                self.check(output)
            path.write_bytes(saved)
            g.write(output / 'smoke.json', original)
        path = output / f['rgb_file']
        renamed = output / 'saved.ppm'
        path.rename(renamed)
        path.symlink_to(renamed)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            self.check(output)

    def test_invalid_native_receipt_is_not_authenticated_or_rendered(self):
        native = self.root / 'native'
        native.mkdir()
        g.write(native / 'manifest.json', {'schema': 1, 'capture_backend': 'synthetic'})
        inputs = self.root / 'list.json'
        g.write(inputs, [self.options])
        with patch.object(g.subprocess, 'run') as process:
            with self.assertRaisesRegex(ValueError, 'provenance'):
                c.run(native, self.root / 'out', inputs, 'light', self.exe, 'test-revision')
            process.assert_not_called()
        self.assertFalse((self.root / 'out').exists())

    def test_real_receipt_process_boundary_integration_and_retained_failure(self):
        native = g.REPO / 'target/glass-native-handoff/run-005'
        if not (native / 'manifest.json').is_file():
            self.skipTest('real run-005 receipt unavailable; no synthetic authentication substitute')
        inputs, output = self.root / 'list.json', self.root / 'out'
        g.write(inputs, [{'regular_gain': 1}, {'regular_gain': 1.1}])

        def process(command, **kwargs):
            plan = g.load(output / 'plan.json')
            self.assertEqual(plan['training'], {'appearance': 'dark', 'phase': 'static', 'id': 'regular-large'})
            self.assertEqual(g.load(command[1])['scale'], g.load(native / 'manifest.json')['scale'])
            render(command, **kwargs)

        with patch.object(g.subprocess, 'run', side_effect=process) as boundary:
            report = c.run(native, output, inputs, 'dark', self.exe, 'test-revision')
            self.assertEqual(boundary.call_count, 2)
            with self.assertRaises(FileExistsError):
                c.run(native, output, inputs, 'dark', self.exe, 'test-revision')
            self.assertEqual(boundary.call_count, 2)
        self.assertEqual(report['chosen_index'], 0)  # Identical synthetic pixels: stable tie.
        self.assertEqual(report['chosen_parameters'], {'regular_gain': 1, 'protect_text': 0})
        self.assertEqual(report['installation_scope'], 'studio-dark ONLY; no production edits performed')
        self.assertEqual(report['identity']['reference_validation']['frames'], 36)
        self.assertEqual(g.load(output / 'calibration.json'), report)
        self.assertFalse((output / 'candidate-000/candidate.json').exists())
        for trial in report['candidates']:
            self.assertEqual(len(trial['pills']), 10)
            self.assertEqual(len(trial['smoke_manifest']['frames']), 4)
        failed = self.root / 'failed'
        with patch.object(g.subprocess, 'run', side_effect=RuntimeError('process failed')):
            with self.assertRaisesRegex(RuntimeError, 'process failed'):
                c.run(native, failed, inputs, 'light', self.exe, 'test-revision')
        self.assertEqual(g.load(failed / 'failure.json')['completed_candidates'], 0)
        self.assertTrue((failed / 'candidate-000/request.json').is_file())
        with self.assertRaises(FileExistsError):
            c.run(native, failed, inputs, 'light', self.exe, 'test-revision')


if __name__ == '__main__':
    unittest.main()
