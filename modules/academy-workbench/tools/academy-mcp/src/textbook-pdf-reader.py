#!/usr/bin/env python3
"""Bounded, text-only reader for explicitly selected textbook PDF pages.

The supported layout has labelled middle/minor/page columns, explicit rectangular
cells, and a printed numbered middle cell containing numbered minor cells. Other
layouts remain candidate text, not inferred structure.
"""
import hashlib
from collections import Counter
import json
import os
import re
import stat
import sys
import unicodedata

MAX_BYTES = 64 * 1024 * 1024
MAX_OUTPUT = 200_000


def valid_request(request):
    if not isinstance(request, dict) or set(request) - {'pages', 'unitCodes', 'scanPages'}:
        raise ValueError('invalid_request')
    pages = request.get('pages')
    if pages is not None and (not isinstance(pages, list) or not 1 <= len(pages) <= 8 or any(type(p) is not int or p < 1 for p in pages) or len(set(pages)) != len(pages)):
        raise ValueError('invalid_pages')
    codes = request.get('unitCodes', [])
    if not isinstance(codes, list) or len(codes) > 16 or any(not isinstance(c, str) or not re.fullmatch(r'[1-9]\d{0,2}-[1-9]\d{0,2}', c) for c in codes) or len(set(codes)) != len(codes):
        raise ValueError('invalid_unit_codes')
    scan = request.get('scanPages', 16)
    if type(scan) is not int or not 1 <= scan <= 24:
        raise ValueError('invalid_scan_pages')
    return pages, codes, scan


def read_pdf(path, expected):
    if not re.fullmatch(r'[0-9a-f]{64}', expected):
        raise ValueError('invalid_sha256')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as f:
        info = os.fstat(f.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_size > MAX_BYTES:
            raise ValueError('invalid_pdf_file')
        data = f.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES:
        raise ValueError('pdf_too_large')
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError('sha256_mismatch')
    return data


def bad_glyph(text):
    return any(unicodedata.category(c) in ('Co', 'Cn', 'Cs') or c == '\ufffd' for c in text)


def inside(bbox, rect):
    # Require a line's center to lie in the actual cell, including multiline text.
    x = (bbox[0] + bbox[2]) / 2
    y = (bbox[1] + bbox[3]) / 2
    return rect[0] <= x <= rect[2] and rect[1] <= y <= rect[3]


def cell_lines(lines, rect):
    return sorted((line for line in lines if inside(line['bbox'], rect)), key=lambda line: (line['bbox'][1], line['bbox'][0]))


def cell_text(lines, rect):
    return ' '.join(x['text'].strip() for x in cell_lines(lines, rect)).strip()


def same(a, b):
    return abs(a - b) < 0.8


def parse_layout(lines, rects, page_number):
    """Return units only from explicit adjacent table cells; all inputs are plain data."""
    rects = list(dict.fromkeys(tuple(round(float(v), 2) for v in r) for r in rects if len(r) == 4 and r[2] > r[0] and r[3] > r[1]))
    headers = []
    for rect in rects:
        if cell_text(lines, rect) == '중단원명':
            minor = [r for r in rects if same(r[0], rect[2]) and same(r[1], rect[1]) and same(r[3], rect[3]) and cell_text(lines, r) == '소단원명']
            if len(minor) != 1:
                continue
            page = [r for r in rects if same(r[0], minor[0][2]) and same(r[1], rect[1]) and same(r[3], rect[3]) and cell_text(lines, r) == '쪽수']
            if len(page) == 1:
                headers.append((rect, minor[0], page[0]))
    units = []
    for h, mh, ph in headers:
        # A heading is accepted only in the bounded region directly above its table.
        headings = [line for line in lines if 0 <= h[1] - line['bbox'][3] <= 35 and re.match(r'^[ⅠⅡⅢⅣⅤⅥⅦⅧⅨⅩ]+\s*\.\s*\S', line['text'].strip())]
        major = headings[0]['text'].strip() if len(headings) == 1 else None
        next_header_y = min((other[0][1] for other in headers if other[0][1] > h[1]), default=float('inf'))
        middles = [r for r in rects if same(r[0], h[0]) and same(r[2], h[2]) and r[1] >= h[3] - 0.8 and r[3] < next_header_y]
        for group in middles:
            group_text = cell_text(lines, group)
            gm = re.fullmatch(r'0*([1-9]\d{0,2})\.\s*(\S.*)', group_text)
            if not gm:
                continue
            rows = sorted((r for r in rects if same(r[0], mh[0]) and same(r[2], mh[2]) and r[1] >= group[1] - 0.8 and r[3] <= group[3] + 0.8), key=lambda r:r[1])
            if not rows or not same(rows[0][1], group[1]) or not same(rows[-1][3], group[3]) or any(not same(a[3], b[1]) for a, b in zip(rows, rows[1:])):
                continue
            for row in rows:
                pages = [r for r in rects if same(r[0], ph[0]) and same(r[2], ph[2]) and same(r[1], row[1]) and same(r[3], row[3])]
                if len(pages) != 1:
                    continue
                title = cell_text(lines, row)
                m = re.fullmatch(r'0*([1-9]\d{0,2})\.\s*(\S.*)', title)
                pg = cell_text(lines, pages[0])
                if not m or not re.fullmatch(r'[1-9]\d{0,4}', pg):
                    continue
                label = m.group(2)
                ambiguous = bad_glyph(label) or bad_glyph(group_text) or (major is not None and bad_glyph(major))
                units.append({'code': f'{int(gm.group(1))}-{int(m.group(1))}', 'title': label, 'majorTitle': major, 'middleTitle': group_text, 'printedPage': int(pg), 'pdfPage': page_number, 'qualification': 'ambiguous_text' if ambiguous else 'text_verified'})
    return units


def run(path, expected, request):
    pages, codes, scan = valid_request(request)
    data = read_pdf(path, expected)
    import pymupdf
    doc = pymupdf.open(stream=data, filetype='pdf')
    if doc.needs_pass:
        raise ValueError('encrypted_pdf')
    if pages is not None and any(p > len(doc) for p in pages):
        raise ValueError('page_out_of_range')
    selected = pages if pages is not None else list(range(1, min(scan, len(doc)) + 1))
    units, candidates, extraction_reasons = [], [], set()
    for number in selected:
        page = doc[number - 1]
        raw = page.get_text('dict', flags=pymupdf.TEXTFLAGS_DICT & ~pymupdf.TEXT_PRESERVE_IMAGES)
        lines = [{'bbox': line['bbox'], 'text': ''.join(span['text'] for span in line['spans'])} for block in raw['blocks'] for line in block.get('lines', [])]
        candidate_text = '\n'.join(line['text'] for line in lines)
        candidate = candidate_text[:3000]
        if len(candidate_text) > 3000 and any(marker in candidate_text for marker in ('중단원명','소단원명','Contents','목차')):
            extraction_reasons.add('candidate_text_truncated')
        # Only geometrically qualified unit cells leave this process. A word
        # such as Contents on a page does not qualify its surrounding body text.
        if len(lines) > 10000 or len(candidate_text) > 200000:
            extraction_reasons.add('page_text_limit_exceeded')
            continue
        if not all(any(line['text'].strip() == h for line in lines) for h in ('중단원명', '소단원명', '쪽수')):
            continue
        rects = [tuple(item[1]) for drawing in page.get_drawings() for item in drawing['items'] if item[0] == 're']
        if len(rects) > 5000:
            extraction_reasons.add('page_geometry_limit_exceeded')
            continue
        units.extend(parse_layout(lines, rects, number))
    # Repeated codes across pages or tables are not resolved by selecting one.
    counts = Counter(u['code'] for u in units)
    duplicates = {code for code, count in counts.items() if count > 1}
    units = [u for u in units if u['code'] not in duplicates and (not codes or u['code'] in codes)]
    missing = [c for c in codes if not any(u['code'] == c for u in units)]
    reasons = ['selected_pages_only'] + sorted(extraction_reasons)
    if missing: reasons.append('requested_codes_unresolved')
    if duplicates: reasons.append('duplicate_codes_unresolved')
    if not units: reasons.append('no_supported_units_found')
    if any(u['qualification'] == 'ambiguous_text' for u in units): reasons.append('ambiguous_glyphs')
    result = {'schemaVersion': 1, 'pdfSha256': expected, 'pageCount': len(doc), 'scannedPages': selected, 'units': units, 'missingUnitCodes': missing, 'coverage': 'selected_pages_only', 'requiresVisualVerification': bool(extraction_reasons or missing or duplicates or not units or any(u['qualification'] == 'ambiguous_text' for u in units)), 'reasonCodes': reasons, 'textCandidates': candidates}
    # Bound bytes, including JSON escaping and non-ASCII. Remove trailing candidates first.
    while len(json.dumps(result, ensure_ascii=True).encode()) > MAX_OUTPUT and result['textCandidates']:
        result['textCandidates'].pop()
        if 'candidate_output_truncated' not in reasons: reasons.append('candidate_output_truncated')
        result['requiresVisualVerification'] = True
    if len(json.dumps(result, ensure_ascii=True).encode()) > MAX_OUTPUT:
        raise ValueError('output_limit_exceeded')
    return result


def main():
    try:
        # Bound native-library allocations before opening/extracting a PDF.
        import resource
        resource.setrlimit(resource.RLIMIT_AS, (1536 * 1024 * 1024, 1536 * 1024 * 1024))
        resource.setrlimit(resource.RLIMIT_CPU, (10, 10))
        if len(sys.argv) != 4 or len(sys.argv[3]) > 4096:
            raise ValueError('invalid_arguments')
        result = run(sys.argv[1], sys.argv[2], json.loads(sys.argv[3]))
        print(json.dumps(result, ensure_ascii=True, separators=(',', ':')))
    except Exception as exc:
        # Do not expose paths, document text, or interpreter exception details.
        code = str(exc) if isinstance(exc, ValueError) and re.fullmatch(r'[a-z_]+', str(exc)) else 'pdf_read_failed'
        print(json.dumps({'error': code}), file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
