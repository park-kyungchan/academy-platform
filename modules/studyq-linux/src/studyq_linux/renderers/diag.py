"""The `diag` official diagnostic report, and the layout it shares with `new_diag` (specs/studyq-linux/reports-batch1.md R1-R4, R7).

`plan_diag` turns a validated input into pages on `image/diag/diag_1.jpg` to `diag_9.jpg`: the
cover, the certificate, the summary (grade picture, scores, score bars, comment), the difficulty
page (question counts, share ring, score per level), the thinking-skill graph, the area table with
its signal lights and rating cards, two pages without input fields, then the question table on
`diag_9.jpg`, one page for each 25 questions. It is pure Python; text metrics and picture sizes
come from the caller. Every box is in template pixels (1 px = 0.24 pt).
"""

from dataclasses import dataclass

from ..course_book import PagePlan, Plan
from ..geometry import px
from . import common
from .common import add, text

REPORT_TYPE = "diag"


@dataclass(frozen=True)
class Style:
    """What `diag` and `new_diag` do not share."""

    template_dir: str
    graph: str
    chart_rows: tuple  # (y0, y1) of the my-score bar and of the average bar, template pixels
    my_color: tuple
    avg_color: tuple
    level_colors: dict
    lights: bool  # the area table shows `5p/light_<signal>.jpg`
    difficulty_chart: bool  # page 4 draws `8p/difficulty_chart.jpg` under the scores

    def template(self, n):
        return self.template_dir + "diag_%d.jpg" % n


OVERLAY_DIR = "image/diag/"
POINT = OVERLAY_DIR + "4p/oneself-point.png"
DIFFICULTY_CHART = OVERLAY_DIR + "8p/difficulty_chart.jpg"
DIAG = Style(
    template_dir="image/diag/",
    graph=OVERLAY_DIR + "4p/area_graph.jpg",
    chart_rows=((1820.0, 1899.0), (1960.0, 2039.0)),
    my_color=(0.84, 0.22, 0.16),
    avg_color=(0.39, 0.67, 0.38),
    level_colors={"top": (0.13, 0.68, 0.27), "high": (0.45, 0.71, 0.2), "mid": (0.68, 0.8, 0.0), "low": (0.7, 0.69, 0.0)},
    lights=True,
    difficulty_chart=True,
)

# Cover and certificate.
COVER_TITLE = (866.0, 1312.0, 2345.0, 1417.0)
COVER_DATE = (1732.0, 3048.0, 2345.0, 3136.0)
COVER_STUDENT = (1732.0, 3141.0, 2345.0, 3229.0)
CERT_STUDENT = (630.0, 1473.0, 2100.0, 1554.0)
# Summary page.
GRADE_AT = (190.0, 380.0)
MY_SCORE = (1295.0, 691.0, 1636.0, 796.0)
AVG_SCORE = (1295.0, 1120.0, 1636.0, 1225.0)
BAR_ZERO_PX, BAR_PX_PER_POINT = 311.5, 18.4975
COMMENT_BOX = (912.0, 2640.0, 2210.0, 3240.0)
# Difficulty page.
COUNT_X = (1715.0, 2108.0)
COUNT_ROWS = ((726.0, 806.0), (898.0, 978.0), (1069.0, 1149.0), (1241.0, 1321.0))
SHARE_RING = (764.75, 988.75, 402.5, 210.0)  # centre x, centre y, outer and inner radius
RING_CENTERS_X = (548.0, 1008.0, 1467.0, 1925.0)
RING_CENTER_Y = 1997.0
RING_HALF_W, RING_HALF_H = 130.0, 60.0
DIFFICULTY_CHART_AT = (281.0, 1775.0)
# Thinking-skill graph: its top left on the page, and in graph pixels the centre and radius of the
# score 100 circle and the bubble centres of the first four areas (left, top, right, bottom axis).
GRAPH_AT = (248.0, 1300.0)
GRAPH_CENTER = (992.0, 995.0)
GRAPH_RADIUS = 378.0
GRAPH_AXES = ((-1.0, 0.0), (0.0, -1.0), (1.0, 0.0), (0.0, 1.0))
GRAPH_BUBBLES = ((165.0, 640.0), (648.0, 163.0), (1334.0, 163.0), (1820.0, 640.0))
# Area table: the printed rows are masked and the areas drawn in the same body.
AREA_BODY = (222.0, 670.0, 2296.0, 1057.0)
AREA_COLUMNS = {"name": (222.0, 933.0), "correct": (948.0, 1333.0), "total": (1348.0, 1736.0), "light": (1750.0, 2296.0)}
RATING_BOX = (201.0, 1531.0, 2258.0, 3273.0)
RATING_CARD = (420.0, 623.0)
RATING_ROW_PX = 800.0
# Question table (`diag_9.jpg`).
QUESTION_TOP_PX, QUESTION_STEP_PX = 334.0, 98.0
QUESTION_COLUMNS = {"no": (247.0, 425.0), "area": (432.0, 1297.0), "difficulty": (1304.0, 1517.0), "correct_rate": (1750.0, 1999.0)}
QUESTION_MARK = (2007.0, 2224.0)
QUESTION_SIZE = 10.5


def grade_rel(grade):
    return OVERLAY_DIR + "2p/lv%02d.jpg" % grade


def light_rel(signal):
    return OVERLAY_DIR + "5p/light_%s.jpg" % signal


def rating_rel(rating):
    return OVERLAY_DIR + "6p/%s.jpg" % rating


def cover(ctx, style, data, title_box=COVER_TITLE):
    page = PagePlan(style.template(1))
    add(page, text(ctx, "p1.title", data["title"], title_box, "l", 18))
    add(page, text(ctx, "p1.test_date", data["test_date"], COVER_DATE, "l", 14))
    add(page, text(ctx, "p1.student", data["student"], COVER_STUDENT, "l", 14))
    return page


def _certificate(ctx, style, data):
    page = PagePlan(style.template(2))
    add(page, text(ctx, "p2.student", data["student"], CERT_STUDENT, "l", 16))
    return page


def _summary(ctx, style, data):
    page = PagePlan(style.template(3))
    page.images.append(common.image_at(ctx, grade_rel(data["grade"]), *GRADE_AT))
    add(page, text(ctx, "p3.total_score", common.number(data["total_score"]), MY_SCORE, "r", 24))
    add(page, text(ctx, "p3.avg_score", common.number(data["avg_score"]), AVG_SCORE, "r", 24))
    for (y0, y1), score, color in zip(style.chart_rows, (data["total_score"], data["avg_score"]), (style.my_color, style.avg_color)):
        common.bar(page, BAR_ZERO_PX, BAR_ZERO_PX + BAR_PX_PER_POINT * score, y0, y1, color)
    page.fields.extend(common.comment_fields(ctx, "p3.comment", data["comment"], COMMENT_BOX))
    return page


def _difficulty(ctx, style, data):
    page = PagePlan(style.template(4))
    diff = data["difficulty"]
    if style.difficulty_chart:
        page.images.append(common.image_at(ctx, DIFFICULTY_CHART, *DIFFICULTY_CHART_AT))
    for k, level in enumerate(common.LEVELS):
        y0, y1 = COUNT_ROWS[k]
        add(page, text(ctx, "p4.%s.questions" % level, str(diff[level]["questions"]), (COUNT_X[0], y0, COUNT_X[1], y1), "r", 16))
        cx = RING_CENTERS_X[k]
        box = (cx - RING_HALF_W, RING_CENTER_Y - RING_HALF_H, cx + RING_HALF_W, RING_CENTER_Y + RING_HALF_H)
        add(page, text(ctx, "p4.%s.score" % level, common.number(diff[level]["score"]), box, "c", 20))
    common.share_rings(page, *SHARE_RING, diff, style.level_colors)
    return page


def area_counts(data):
    """(correct, total) questions of each area, by the question's `area` equal to the area's name."""
    counts = []
    for area in data["areas"]:
        mine = [q for q in data["questions"] if q["area"] == area["name"]]
        counts.append((sum(1 for q in mine if q["correct"]), len(mine)))
    return counts


def _graph(ctx, style, data):
    page = PagePlan(style.template(5))
    gx, gy = GRAPH_AT
    page.images.append(common.image_at(ctx, style.graph, gx, gy))
    counts = area_counts(data)
    for i, area in enumerate(data["areas"][: len(GRAPH_AXES)]):
        dx, dy = GRAPH_AXES[i]
        r = GRAPH_RADIUS * area["score"] / 100.0
        page.images.append(common.marker(ctx, POINT, gx + GRAPH_CENTER[0] + dx * r, gy + GRAPH_CENTER[1] + dy * r))
        bx, by = GRAPH_BUBBLES[i]
        box = (gx + bx - 110, gy + by - 90, gx + bx + 110, gy + by + 40)
        add(page, text(ctx, "p5.a%d.questions" % i, str(counts[i][1]), box, "c", 20))
    return page


def _areas(ctx, style, data):
    page = PagePlan(style.template(6))
    common.mask(page, AREA_BODY)
    x0, top, x1, bottom = AREA_BODY
    step = (bottom - top) / len(data["areas"])
    size = min(11.0, round(px(step) * 0.7, 1))
    counts = area_counts(data)
    for i, area in enumerate(data["areas"]):
        y0 = top + i * step
        values = {"name": area["name"], "correct": str(counts[i][0]), "total": str(counts[i][1])}
        for name, value in values.items():
            c0, c1 = AREA_COLUMNS[name]
            add(page, text(ctx, "p6.a%d.%s" % (i, name), value, (c0 + 20, y0, c1 - 20, y0 + step), "l" if name == "name" else "c", size))
        if style.lights:
            c0, c1 = AREA_COLUMNS["light"]
            page.images.append(common.fit_image(ctx, light_rel(area["signal"]), (c0, y0 + 4, c1, y0 + step - 4)))
    bx0, by0, bx1, _ = RATING_BOX
    cell = (bx1 - bx0) / 4
    card_w, card_h = RATING_CARD
    for i, area in enumerate(data["areas"]):
        cx = bx0 + cell * (i % 4) + cell / 2
        y0 = by0 + 70 + RATING_ROW_PX * (i // 4)
        page.images.append(common.fit_image(ctx, rating_rel(area["rating"]), (cx - card_w / 2, y0, cx + card_w / 2, y0 + card_h)))
        add(page, text(ctx, "p6.a%d.caption" % i, area["name"], (cx - cell / 2 + 10, y0 + card_h + 10, cx + cell / 2 - 10, y0 + card_h + 70), "c", 10))
    return page


def question_pages(ctx, data, template_of, number_of, group="area"):
    """The question table: 25 rows a page from `QUESTION_TOP_PX`; chunk `n` is drawn on `template_of(n)` as page `number_of(n)`."""
    pages = []
    for n, chunk in enumerate(common.question_chunks(data["questions"])):
        page = PagePlan(template_of(n))
        prefix = "p%d" % number_of(n)
        for i, q in enumerate(chunk):
            index = n * common.QUESTIONS_PER_PAGE + i
            y0 = QUESTION_TOP_PX + QUESTION_STEP_PX * i
            values = {"no": str(q["no"]), "area": q[group], "difficulty": q["difficulty"], "correct_rate": common.number(q["correct_rate"])}
            for name, value in values.items():
                c0, c1 = QUESTION_COLUMNS[name]
                add(page, text(ctx, "%s.q%d.%s" % (prefix, index, name), value, (c0, y0, c1, y0 + QUESTION_STEP_PX), "l" if name == "area" else "c", QUESTION_SIZE))
            page.images.append(common.mark_image(ctx, q["correct"], QUESTION_MARK, y0, QUESTION_STEP_PX))
        pages.append(page)
    return pages


def fixed_pages(ctx, style, data):
    """Pages 1 to 8 of `diag` and `new_diag`: pages 7 and 8 carry no field of the input."""
    return [
        cover(ctx, style, data),
        _certificate(ctx, style, data),
        _summary(ctx, style, data),
        _difficulty(ctx, style, data),
        _graph(ctx, style, data),
        _areas(ctx, style, data),
        PagePlan(style.template(7)),
        PagePlan(style.template(8)),
    ]


def plan_diag(data, ctx):
    return Plan(fixed_pages(ctx, DIAG, data) + question_pages(ctx, data, lambda n: DIAG.template(9), lambda n: 9 + n))


def template_sequence(question_count):
    pages = -(-question_count // common.QUESTIONS_PER_PAGE)
    return [DIAG.template(n) for n in range(1, 9)] + [DIAG.template(9)] * pages


def required_style_assets(style, data):
    rels = {style.template(n) for n in range(1, 10)}
    rels.update({grade_rel(data["grade"]), style.graph, POINT, common.MARK_O, common.MARK_X})
    rels.update(rating_rel(a["rating"]) for a in data["areas"])
    if style.lights:
        rels.update(light_rel(a["signal"]) for a in data["areas"])
    if style.difficulty_chart:
        rels.add(DIFFICULTY_CHART)
    return rels


def required_diag_assets(data):
    """Relative paths of every asset a plan for `data` reads."""
    return sorted(required_style_assets(DIAG, data))
