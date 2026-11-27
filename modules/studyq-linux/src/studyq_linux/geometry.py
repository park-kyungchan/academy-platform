"""libHaru-to-PyMuPDF coordinates and chart primitives (specs/studyq-linux/core.md R15, R29).

libHaru puts the origin at the bottom left of the page in points; PyMuPDF at the top left.
A template pixel is 72/300 pt (A4 at 300 dpi, 3508 px tall). Positions inside a template or a
chart image are given in pixels from its top left.
"""

import math

PX = 72.0 / 300.0
PAGE_W = 595.2756
PAGE_H = 841.8898  # H_PT: y_fitz = PAGE_H - y_hpdf


def px(value):
    return value * PX


def hpdf_to_fitz_y(y_hpdf):
    return PAGE_H - y_hpdf


def box_px(x0, y0, x1, y1):
    return (px(x0), px(y0), px(x1), px(y1))


class ChartImage:
    """A chart picture placed at (x_pt, y_hpdf); score 0 and 100 sit at fixed rows of the picture."""

    def __init__(self, rel, x_pt, y_hpdf, y0_px=904.5, y100_px=144.5):
        self.rel = rel
        self.x_pt = x_pt
        self.y_hpdf = y_hpdf
        self.y0_px = y0_px
        self.y100_px = y100_px

    @property
    def top_pt(self):
        return hpdf_to_fitz_y(self.y_hpdf)

    def y_px(self, score):
        return self.y0_px + (self.y100_px - self.y0_px) * (score / 100.0)

    def point(self, x_px, score):
        """Page position (pt, PyMuPDF) of a score at a picture column, with no offset."""
        return (self.x_pt + px(x_px), self.top_pt + px(self.y_px(score)))

    def rect(self, size_px):
        w, h = size_px
        return (self.x_pt, self.top_pt, self.x_pt + px(w), self.top_pt + px(h))


def centered_rect(center, size_px):
    w, h = size_px
    return (center[0] - px(w) / 2, center[1] - px(h) / 2, center[0] + px(w) / 2, center[1] + px(h) / 2)


def wedge_points(cx, cy, radius, start_deg, end_deg, step=3.0):
    """Polygon of a pie wedge, clockwise from the top (start 0 deg is 12 o'clock)."""
    points = [(cx, cy)]
    steps = max(1, int(math.ceil((end_deg - start_deg) / step)))
    for k in range(steps + 1):
        angle = math.radians(start_deg + (end_deg - start_deg) * k / steps)
        points.append((cx + radius * math.sin(angle), cy - radius * math.cos(angle)))
    return points
