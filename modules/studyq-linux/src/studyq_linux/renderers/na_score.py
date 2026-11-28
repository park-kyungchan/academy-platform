"""The `na_score` official NA score report (specs/studyq-linux/reports-batch2.md R1-R3, R5, R7).

`plan_na_score` turns a validated input into pages on `image/na_score/score_1.jpg` to `score_3.jpg`:
the cover, the results page (the rank-band picture `0<rank_band>.jpg`, the score and the average with
a bar each, the unit table, the comment), then the question table on `score_3.jpg`, one page for
each 25 questions, its rows placed as on the `diag` question table with the unit in the area column.
Every box is in template pixels (1 px = 0.24 pt).
"""

from ..course_book import PagePlan, Plan
from ..geometry import px
from . import common
from .common import add, text
from .diag import question_pages
from .formative import COVER_TITLE

REPORT_TYPE = "na_score"
TEMPLATE_DIR = "image/na_score/"

MY_COLOR = (0.12, 0.48, 0.77)
AVG_COLOR = (0.78, 0.78, 0.78)
# Cover.
COVER_DATE = (1732.0, 3048.0, 2345.0, 3136.0)
COVER_STUDENT = (1732.0, 3141.0, 2345.0, 3229.0)
# Results page.
BAND_BOX = (212.0, 400.0, 1200.0, 1200.0)
MY_SCORE = (1330.0, 639.0, 1654.0, 744.0)
AVG_SCORE = (1330.0, 1033.0, 1654.0, 1138.0)
BAR_ZERO_PX, BAR_PX_PER_POINT = 309.5, 18.545
BAR_ROWS = ((1320.0, 1382.0), (1452.0, 1514.0))
UNIT_BODY = (212.0, 1600.0, 2269.0, 2464.0)
UNIT_COLUMNS = {"name": (212.0, 1420.0), "my_score": (1420.0, 1845.0), "average": (1845.0, 2269.0)}
UNIT_ROW_MAX_PX = 98.0
COMMENT_BOX = (245.0, 2560.0, 2240.0, 3260.0)


def template(n):
    return TEMPLATE_DIR + "score_%d.jpg" % n


def band_rel(rank_band):
    return TEMPLATE_DIR + "0%d.jpg" % rank_band


def _cover(ctx, data):
    page = PagePlan(template(1))
    add(page, text(ctx, "p1.title", data["title"], COVER_TITLE, "l", 18))
    add(page, text(ctx, "p1.test_date", data["test_date"], COVER_DATE, "l", 14))
    add(page, text(ctx, "p1.student", data["student"], COVER_STUDENT, "l", 14))
    return page


def _results(ctx, data):
    page = PagePlan(template(2))
    page.images.append(common.fit_image(ctx, band_rel(data["rank_band"]), BAND_BOX))
    add(page, text(ctx, "p2.my_score", common.number(data["my_score"]), MY_SCORE, "r", 24))
    add(page, text(ctx, "p2.avg_score", common.number(data["avg_score"]), AVG_SCORE, "r", 24))
    for (y0, y1), score, color in zip(BAR_ROWS, (data["my_score"], data["avg_score"]), (MY_COLOR, AVG_COLOR)):
        common.bar(page, BAR_ZERO_PX, BAR_ZERO_PX + BAR_PX_PER_POINT * score, y0, y1, color)
    x0, top, x1, bottom = UNIT_BODY
    step = min(UNIT_ROW_MAX_PX, (bottom - top) / len(data["units"]))
    size = min(11.0, round(px(step) * 0.7, 1))
    for i, unit in enumerate(data["units"]):
        y0 = top + step * i
        values = {"name": unit["name"], "my_score": common.number(unit["my_score"]), "average": common.number(unit["average"])}
        for name, value in values.items():
            c0, c1 = UNIT_COLUMNS[name]
            add(page, text(ctx, "p2.u%d.%s" % (i, name), value, (c0 + 20, y0, c1 - 20, y0 + step), "l" if name == "name" else "c", size))
    page.fields.extend(common.comment_fields(ctx, "p2.comment", data["comment"], COMMENT_BOX))
    return page


def plan_na_score(data, ctx):
    questions = question_pages(ctx, data, lambda n: template(3), lambda n: 3 + n, group="unit")
    return Plan([_cover(ctx, data), _results(ctx, data)] + questions)


def template_sequence(question_count):
    pages = -(-question_count // common.QUESTIONS_PER_PAGE)
    return [template(1), template(2)] + [template(3)] * pages


def required_na_score_assets(data):
    """Relative paths of every asset a plan for `data` reads."""
    return sorted({template(1), template(2), template(3), band_rel(data["rank_band"]), common.MARK_O, common.MARK_X})
