import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

PATH = Path(__file__).resolve().parents[1] / 'src' / 'textbook-pdf-reader.py'
spec = importlib.util.spec_from_file_location('textbook_pdf_reader', PATH)
reader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reader)


def fixture():
    # Synthetic table: the second group contains only one row, the first three.
    rects = [(0, 10, 100, 20), (100, 10, 200, 20), (200, 10, 240, 20),
             (0, 20, 100, 50), (0, 50, 100, 60)]
    lines = []
    def add(x, y, value):
        lines.append({'bbox': (x, y, x + 8, y + 4), 'text': value})
    add(5, 12, '중단원명'); add(105, 12, '소단원명'); add(205, 12, '쪽수')
    add(5, 32, '01. Group A'); add(5, 52, '02. Group B')
    for index in range(4):
        top = 20 + 10 * index
        rects.extend([(100, top, 200, top + 10), (200, top, 240, top + 10)])
        add(105, top + 2, f'{index + 1}. Topic {index + 1}')
        add(205, top + 2, str(10 + index))
    return lines, rects


class LayoutTests(unittest.TestCase):
    def test_unequal_merged_cells_use_exact_geometry(self):
        lines, rects = fixture()
        units = reader.parse_layout(lines, rects, 7)
        self.assertEqual([u['code'] for u in units], ['1-1', '1-2', '1-3', '2-4'])
        self.assertEqual([u['middleTitle'] for u in units], ['01. Group A'] * 3 + ['02. Group B'])
        self.assertEqual([u['printedPage'] for u in units], [10, 11, 12, 13])

    def test_missing_or_private_glyph_is_ambiguous(self):
        lines, rects = fixture()
        lines[-2]['text'] = '4. Formula \ue000\ufffd'
        unit = reader.parse_layout(lines, rects, 7)[-1]
        self.assertEqual(unit['qualification'], 'ambiguous_text')
        self.assertIn('\ue000', unit['title'])

    def test_no_merge_cell_or_header_does_not_guess(self):
        lines, rects = fixture()
        rects.remove((0, 20, 100, 50))
        self.assertEqual([x['code'] for x in reader.parse_layout(lines, rects, 7)], ['2-4'])
        lines[0]['text'] = 'Different layout'
        self.assertEqual(reader.parse_layout(lines, rects, 7), [])

    def test_incomplete_row_geometry_rejects_entire_group(self):
        lines, rects = fixture()
        rects.remove((100, 30, 200, 40))
        self.assertEqual([x['code'] for x in reader.parse_layout(lines, rects, 7)], ['2-4'])


class BoundaryTests(unittest.TestCase):
    def test_request_bounds_and_types(self):
        self.assertEqual(reader.valid_request({}), (None, [], 16))
        for request in ({'pages': [0]}, {'pages': list(range(1, 10))}, {'pages': [True]}, {'unitCodes': ['01-1']}, {'unitCodes': ['1-1'] * 17}, {'scanPages': 25}, {'scanPages': True}, {'other': 1}):
            with self.subTest(request=request), self.assertRaises(ValueError):
                reader.valid_request(request)

    def test_file_hash_and_nofollow(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / 'doc.pdf'
            target.write_bytes(b'synthetic')
            digest = hashlib.sha256(b'synthetic').hexdigest()
            self.assertEqual(reader.read_pdf(str(target), digest), b'synthetic')
            with self.assertRaisesRegex(ValueError, 'sha256_mismatch'):
                reader.read_pdf(str(target), '0' * 64)
            link = Path(tmp) / 'link.pdf'
            link.symlink_to(target)
            with self.assertRaises(OSError):
                reader.read_pdf(str(link), digest)

    def test_oversize_file_rejected_before_reading(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / 'large.pdf'
            with target.open('wb') as file:
                file.truncate(reader.MAX_BYTES + 1)
            with self.assertRaisesRegex(ValueError, 'invalid_pdf_file'):
                reader.read_pdf(str(target), '0' * 64)


if __name__ == '__main__':
    unittest.main()
