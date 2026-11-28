"""Layout pieces shared by the `diag`, `new_diag` and `formative` reports (specs/studyq-linux/reports-batch1.md).

Boxes are given in template pixels (A4 at 300 dpi, 1 px = 0.24 pt) and turned into PyMuPDF points.
A text is a `Field` only when it is not empty, so the parity check never searches for ''.
"""

import math

from ..course_book import PAD_PT, Field, ImageOp, ShapeOp
from ..geometry import box_px, px
from ..textutil import fit_line, format_number, wrap_text

MIN_SIZE = 6.0
MARK_O = "image/mark_o.jpg"
MARK_X = "image/mark_x.jpg"
WHITE = (1.0, 1.0, 1.0)
QUESTIONS_PER_PAGE = 25
LEVELS = ("top", "high", "mid", "low")


def text(ctx, fid, value, box, align, size):
    """One line in a box of template pixels: shrunk down to 6 pt, then cut with `...`; None when empty."""
    if not value:
        return None
    pt = box_px(*box)
    shown, final = fit_line(ctx.measure, value, size, pt[2] - pt[0] - 2 * PAD_PT, MIN_SIZE)
    return Field(fid, shown, pt, align, final)


def add(page, item):
    if item is not None:
        page.fields.append(item)


def number(value):
    return format_number(value)


def comment_fields(ctx, prefix, comment, box, size=11.0):
    """The comment wrapped into a box of template pixels, one field a line, cut with `...` at the end of the box."""
    if not comment:
        return []
    left, top, right, bottom = box
    line_px = size * 1.6 / px(1)
    width = px(right - left) - 2 * PAD_PT
    lines = [line.strip() for line in wrap_text(ctx.measure, comment, size, width)]
    capacity = max(1, int((bottom - top) // line_px))
    if len(lines) > capacity:
        lines = lines[:capacity]
        lines[-1] = fit_line(ctx.measure, lines[-1] + "...", size, width, min_size=size)[0]
    fields = []
    for k, line in enumerate(lines):
        y0 = top + k * line_px
        item = text(ctx, "%s.%d" % (prefix, k), line, (left, y0, right, y0 + line_px), "l", size)
        if item is not None:
            fields.append(item)
    return fields


def fit_image(ctx, rel, box):
    """`rel` inside a box of template pixels, centred, keeping its aspect ratio and never enlarged."""
    w, h = ctx.image_size(rel)
    x0, y0, x1, y1 = box
    scale = min(1.0, (x1 - x0) / w, (y1 - y0) / h)
    cw, ch = w * scale, h * scale
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    return ImageOp(rel, box_px(cx - cw / 2, cy - ch / 2, cx + cw / 2, cy + ch / 2))


def image_at(ctx, rel, x, y):
    """`rel` at its own size with its top left at (x, y) template pixels."""
    w, h = ctx.image_size(rel)
    return ImageOp(rel, box_px(x, y, x + w, y + h))


def marker(ctx, rel, cx, cy):
    """`rel` at its own size centred on (cx, cy) template pixels."""
    w, h = ctx.image_size(rel)
    return ImageOp(rel, box_px(cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2))


def bar(page, x0, x1, y0, y1, color):
    if x1 > x0 and y1 > y0:
        page.shapes.append(ShapeOp("rect", box_px(x0, y0, x1, y1), color))


def mask(page, box):
    """A white rectangle drawn over the template before any picture (it hides printed table rows)."""
    page.shapes.append(ShapeOp("mask", box_px(*box), WHITE))


def ring_points(cx, cy, r_out, r_in, start_deg, end_deg, step=3.0):
    """Polygon (pt) of a ring segment between two radii (template pixels), clockwise from 12 o'clock."""
    steps = max(1, int(math.ceil((end_deg - start_deg) / step)))
    angles = [math.radians(start_deg + (end_deg - start_deg) * k / steps) for k in range(steps + 1)]
    outer = [(px(cx + r_out * math.sin(a)), px(cy - r_out * math.cos(a))) for a in angles]
    inner = [(px(cx + r_in * math.sin(a)), px(cy - r_in * math.cos(a))) for a in reversed(angles)]
    return tuple(outer + inner)


def share_rings(page, cx, cy, r_out, r_in, difficulty, colors):
    """One ring segment for each level with questions, in the order top, high, mid, low."""
    total = sum(difficulty[lv]["questions"] for lv in LEVELS)
    start = 0.0
    for level in LEVELS:
        count = difficulty[level]["questions"]
        if not total or not count:
            continue
        end = start + 360.0 * count / total
        page.shapes.append(ShapeOp("poly", ring_points(cx, cy, r_out, r_in, start, end), colors[level]))
        start = end


def question_chunks(questions):
    return [questions[k : k + QUESTIONS_PER_PAGE] for k in range(0, len(questions), QUESTIONS_PER_PAGE)]


def mark_image(ctx, correct, column, top, step):
    """The O or X mark centred in a result cell (template pixels)."""
    rel = MARK_O if correct else MARK_X
    x0, x1 = column
    return marker(ctx, rel, (x0 + x1) / 2, top + step / 2)
