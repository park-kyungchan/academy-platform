"""The `cism` official weekly or monthly CISM report: layout as data (specs/studyq-linux/core.md R18, R19).

`plan_cism` turns a validated input into pages: page 1 with the header fields and up to 31 chart
rows, chart overflow pages of 38 rows, then narrative pages on `cism_3.jpg` that hold the section
banners and the framed, wrapped text of the sections in the order of their (group, slot). It is
pure Python; text metrics and image sizes come from the caller.
"""

from ..course_book import PAD_PT, Field, ImageOp, PagePlan, Plan, ShapeOp
from ..geometry import box_px, px
from ..textutil import fit_line, wrap_text

REPORT_TYPE = "cism"
TEMPLATE_DIR = "image/cism/"
PAGE1 = TEMPLATE_DIR + "cism_1.jpg"
OVERFLOW = TEMPLATE_DIR + "cism_2.jpg"
NARRATIVE = TEMPLATE_DIR + "cism_3.jpg"

HEADER_FIELDS = (
    ("p1.week_label", "week_label", (46.79, 146.4, 335.0, 165.6), "l"),
    ("p1.teacher", "teacher", (343.1, 146.4, 467.9, 165.6), "l"),
    ("p1.period", "period", (467.9, 146.4, 545.9, 165.6), "r"),
)
HEADER_SIZE = 11.0

CHART_COLUMNS_PX = (
    ("group", 195.0, 657.0, "l"),
    ("student", 657.0, 1063.0, "c"),
    ("class_mode", 1063.0, 1469.0, "c"),
    ("think_note", 1469.0, 1876.0, "c"),
    ("week_test", 1876.0, 2283.0, "c"),
)
CHART_SIZE = 10.5
CHART_STEP_PX = 75.0
FIRST_PAGE_TOP_PX = 920.0
FIRST_PAGE_ROWS = 31
OVERFLOW_TOP_PX = 360.0
OVERFLOW_ROWS = 38

LEFT = 46.79
RIGHT = 547.77
TOP = 120.2
BOTTOM = 792.0
BOX_PAD = 6.0
LINE_H = 16.8
TEXT_SIZE = 10.5
BANNER_GAP = 6.0
SECTION_GAP = 12.0
FRAME_COLOR = (0.55, 0.55, 0.55)
FRAME_WIDTH = 0.75
EPS = 1e-6


def chart_rows_per_page(row_count):
    """Row counts of the chart pages: 31 on page 1, then 38 on each overflow page."""
    counts = [min(row_count, FIRST_PAGE_ROWS)]
    rest = row_count - counts[0]
    while rest > 0:
        counts.append(min(rest, OVERFLOW_ROWS))
        rest -= counts[-1]
    return counts


def banner_rel(group, slot):
    return TEMPLATE_DIR + "%d_%d.jpg" % (group, slot)


def _cell(ctx, fid, text, box, align, size):
    if not text:  # an empty cell is no field: the parity check never searches for ''
        return None
    shown, final = fit_line(ctx.measure, text, size, box[2] - box[0] - 2 * PAD_PT)
    return Field(fid, shown, box, align, final)


def _chart_page(ctx, template, rows, first_index, top_px, prefix):
    page = PagePlan(template)
    for i, row in enumerate(rows):
        y0 = top_px + i * CHART_STEP_PX
        for name, x0, x1, align in CHART_COLUMNS_PX:
            item = _cell(ctx, "%s.r%d.%s" % (prefix, first_index + i, name), row[name], box_px(x0, y0, x1, y0 + CHART_STEP_PX), align, CHART_SIZE)
            if item is not None:
                page.fields.append(item)
    return page


def _chart_pages(ctx, data):
    rows = data["chart_data"]
    first = _chart_page(ctx, PAGE1, rows[:FIRST_PAGE_ROWS], 0, FIRST_PAGE_TOP_PX, "p1")
    for fid, member, box, align in HEADER_FIELDS:
        item = _cell(ctx, fid, data[member], box, align, HEADER_SIZE)
        if item is not None:
            first.fields.append(item)
    pages = [first]
    start, number = FIRST_PAGE_ROWS, 2
    while start < len(rows):
        pages.append(_chart_page(ctx, OVERFLOW, rows[start : start + OVERFLOW_ROWS], start, OVERFLOW_TOP_PX, "p%d" % number))
        start += OVERFLOW_ROWS
        number += 1
    return pages


def _section_lines(ctx, text):
    if not text:
        return []
    width = (RIGHT - LEFT) - 2 * BOX_PAD - 2 * PAD_PT
    lines = [line.strip() for line in wrap_text(ctx.measure, text, TEXT_SIZE, width)]
    while lines and not lines[-1]:
        lines.pop()
    return lines


def _narrative_pages(ctx, data):
    pages = []
    page = None
    y = TOP

    def fresh():
        nonlocal page, y
        page = PagePlan(NARRATIVE)
        pages.append(page)
        y = TOP

    fresh()
    for item in sorted(data["report_list"], key=lambda s: (s["group"], s["slot"])):
        sid = "s%d_%d" % (item["group"], item["slot"])
        rel = banner_rel(item["group"], item["slot"])
        banner_w, banner_h_px = ctx.image_size(rel)
        banner_h = px(banner_h_px)
        if y > TOP + EPS and y + banner_h + BANNER_GAP + 2 * BOX_PAD + LINE_H > BOTTOM + EPS:
            fresh()
        page.images.append(ImageOp(rel, (LEFT, y, LEFT + px(banner_w), y + banner_h)))
        y += banner_h + BANNER_GAP
        lines = _section_lines(ctx, item["text"])
        if not lines:
            page.shapes.append(ShapeOp("frame", (LEFT, y, RIGHT, y + 2 * BOX_PAD + LINE_H), FRAME_COLOR))
            y += 2 * BOX_PAD + LINE_H + SECTION_GAP
            continue
        index = 0
        while True:
            capacity = max(1, int((BOTTOM - y - 2 * BOX_PAD + EPS) // LINE_H))
            chunk = lines[index : index + capacity]
            page.shapes.append(ShapeOp("frame", (LEFT, y, RIGHT, y + 2 * BOX_PAD + len(chunk) * LINE_H), FRAME_COLOR))
            for k, line in enumerate(chunk):
                if line:
                    y0 = y + BOX_PAD + k * LINE_H
                    box = (LEFT + BOX_PAD, y0, RIGHT - BOX_PAD, y0 + LINE_H)
                    page.fields.append(Field("cism.%s.l%d" % (sid, index + k), line, box, "l", TEXT_SIZE))
            y += 2 * BOX_PAD + len(chunk) * LINE_H
            index += len(chunk)
            if index >= len(lines):
                break
            fresh()
        y += SECTION_GAP
    return pages


def plan_cism(data, ctx):
    return Plan(_chart_pages(ctx, data) + _narrative_pages(ctx, data))


def required_cism_assets(data):
    """Relative paths of every asset a plan for `data` reads."""
    rels = {PAGE1, NARRATIVE}
    if len(data["chart_data"]) > FIRST_PAGE_ROWS:
        rels.add(OVERFLOW)
    rels.update(banner_rel(s["group"], s["slot"]) for s in data["report_list"])
    return sorted(rels)
