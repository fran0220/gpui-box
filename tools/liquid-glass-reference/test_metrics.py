"""Independent asymmetric pixel oracles; synthetic files are not native evidence."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

import metrics
import test_validate
from validate import digest


def image(width, height, fn):
    return width, height, bytes(v for y in range(height) for x in range(width) for v in fn(x, y))


class PixelTests(unittest.TestCase):
    def test_asymmetric_rgb_gradient_and_background(self):
        r = image(3, 2, lambda x, y: (10*x+40*y, 20+x, 30+2*y))
        c = image(3, 2, lambda x, y: (12*x+40*y+3, 18+x, 35+2*y))
        bg = image(3, 2, lambda x, y: (2*x+4*y, 10, 20))
        result = metrics.region(r, c, bg, bg, [(0, 0), (1, 0), (1, 1)], 1, 8)
        self.assertEqual(result['rgb_mae'], [13/3, 2, 5])
        self.assertEqual(result['rgb_bias'], [13/3, -2, 5])
        self.assertAlmostEqual(result['gradient_mae'], 1/3)
        self.assertAlmostEqual(result['reference']['background_regression_rgb'][0], 60/7)
        self.assertIsNone(result['reference']['background_regression_rgb'][1])

    def test_checker_signed_channels_and_transmission(self):
        bg = image(16, 8, lambda x, y: (20, 80, 120) if x < 8 else (100, 40, 60))
        r = image(16, 8, lambda x, y: (20, 50, 70) if x < 8 else (60, 30, 40))
        c = image(16, 8, lambda x, y: (100, 60, 90) if x < 8 else (20, 80, 120))
        out = metrics.region(r, c, bg, bg, [(x, 3) for x in range(16)], 1, 8)
        self.assertEqual(out['reference']['checker_contrast_rgb'], [-40, 20, 30])
        self.assertEqual(out['candidate']['checker_contrast_rgb'], [80, -20, -30])
        self.assertEqual(out['reference']['background_regression_rgb'], [.5, .5, .5])
        self.assertEqual(out['candidate']['background_regression_rgb'], [-1, -.5, -.5])

    def test_capsule_excludes_glyph_band_and_corners_not_rim(self):
        interior, rim = metrics.pill_masks([10, 10, 60, 40])
        self.assertIn((40, 16), interior)
        self.assertNotIn((40, 17), interior)  # foreground exclusion starts here
        self.assertNotIn((10, 10), interior)
        self.assertNotIn((10, 10), rim)  # rounded, not a rectangular rim
        self.assertIn((40, 9), rim)
        self.assertFalse(set(interior) & set(rim))
        bg = image(80, 60, lambda x, y: (7, 11, 19))
        # Poison a glyph and a corner; only the independently chosen top-strip
        # pixel must contribute to the interior error.
        c = image(80, 60, lambda x, y: (37, 11, 19) if (x, y) in {(40, 16), (40, 30), (10, 10)} else (7, 11, 19))
        out = metrics.region(bg, c, bg, bg, interior, 1, 8)
        self.assertEqual(out['rgb_mae'], [30/len(interior), 0, 0])

    def test_morphology_uses_independent_backgrounds_and_exclusion(self):
        rb = image(5, 3, lambda x, y: (10, 20, 30))
        cb = image(5, 3, lambda x, y: (50, 60, 70))
        r = image(5, 3, lambda x, y: (40, 50, 60) if (x, y) in {(0, 0), (1, 0), (4, 2)} else (10, 20, 30))
        c = image(5, 3, lambda x, y: (80, 90, 100) if (x, y) in {(1, 0), (1, 1), (4, 2)} else (50, 60, 70))
        out = metrics.morphology(r, c, rb, cb, [(0, 0), (1, 0), (1, 1), (3, 2)], 1)
        self.assertEqual(out['iou'], 1/3)
        self.assertEqual(out['symmetric_difference'], 2)
        self.assertEqual(out['reference']['centroid'], [1, .5])
        self.assertEqual(out['candidate']['bounds'], [1, 0, 2, 2])

    def test_transition_swept_glyph_envelope(self):
        points = metrics.transition_mask({'button': [64, 448, 144, 48], 'menu': [64, 448, 272, 128]})
        for p in [(136, 472), (200, 490), (200, 540), (170, 520)]:
            self.assertNotIn(p, points)
        for p in [(65, 480), (330, 510), (200, 449), (200, 575)]:
            self.assertIn(p, points)

    def test_sampling_scale_and_bad_mask(self):
        im = image(6, 4, lambda x, y: (x, y, x+10*y))
        self.assertEqual(metrics.sample(im, (1, 0), 2), (3, 1, 13))
        with self.assertRaises(ValueError):
            metrics.sample(im, (6, 0), 1)

    def test_near_far_masks_and_residual_difference(self):
        near = metrics.bridge_mask([[2, 2, 12, 20], [22, 2, 12, 20]])
        far = metrics.bridge_mask([[2, 26, 12, 20], [38, 26, 12, 20]])
        self.assertEqual(set(near), {(x, y) for x in range(16, 20) for y in range(10, 14)})
        self.assertEqual(len(far), 80)
        bg = image(52, 48, lambda x, y: (11, 21, 31))
        r = image(52, 48, lambda x, y: (41, 51, 61) if 16 <= x < 20 and 10 <= y < 14 else (11, 21, 31))
        c = image(52, 48, lambda x, y: (20, 30, 40) if 16 <= x < 36 and 34 <= y < 38 else (11, 21, 31))
        n = metrics.region(r, c, bg, bg, near, 1, 8)
        f = metrics.region(r, c, bg, bg, far, 1, 8)
        self.assertEqual(n['reference']['background_residual_mae'] - f['reference']['background_residual_mae'], 30)
        self.assertEqual(n['candidate']['background_residual_mae'] - f['candidate']['background_residual_mae'], -9)

    def test_strict_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'report.json'
            for content in ('{"x":1,"x":2}', '{"x":NaN}', '{'):
                p.write_text(content)
                with self.assertRaises(ValueError):
                    metrics.read_json(p)


class BundleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        test_validate.EvidenceTests.setUpClass()
        cls.root = test_validate.EvidenceTests.root
        cls.reference = test_validate.EvidenceTests.original
        (cls.root / 'manifest.json').write_text(json.dumps(cls.reference))
        (cls.root / 'parameters.json').write_text('{}')
        fixture_test = test_validate.EvidenceTests()
        fixture_test.m = cls.reference
        cls.candidate = fixture_test.candidate()
        cls.candidate['parameters_sha256'] = digest(cls.root / 'parameters.json')

    @classmethod
    def tearDownClass(cls):
        test_validate.EvidenceTests.tearDownClass()

    def test_complete_and_splits(self):
        (self.root / 'candidate.json').write_text(json.dumps(self.candidate))
        out = metrics.analyze(self.root, self.root)
        self.assertEqual(out['pairs'], 36)
        self.assertEqual([(p['id'], p['appearance']) for p in out['pills'] if p['split'] == 'training'], [('regular-large', 'light')])
        self.assertTrue(all(p['split'] == 'held-out' for p in out['pills'] if p['appearance'] == 'dark' or p['id'] == 'regular-small'))
        self.assertTrue(all(p['interior']['mae'] == 0 for p in out['pills']))

    def test_missing_hash_dimension_and_timing_refused(self):
        for mutate in (lambda c: c['frames'].pop(),
                       lambda c: c['frames'][0].update(rgb_file_sha256='0'*64),
                       lambda c: c['frames'][0].update(rgb_file='absent.ppm'),
                       lambda c: c['frames'][0].update(pixel_size=[640, 960]),
                       lambda c: c.update(scale=2),
                       lambda c: c['frames'][2].update(sample_time_after_trigger=99),
                       lambda c: c.update(parameters_sha256='0'*64),
                       lambda c: c['frames'][0].update(index=False)):
            with self.subTest(mutate=mutate):
                candidate = copy.deepcopy(self.candidate)
                mutate(candidate)
                (self.root / 'candidate.json').write_text(json.dumps(candidate))
                with self.assertRaises((ValueError, OSError)):
                    metrics.analyze(self.root, self.root)


if __name__ == '__main__':
    unittest.main()
