"""The `formative` official formative-assessment report (specs/studyq-linux/reports-batch1.md R1-R3, R6, R7).

`plan_formative` turns a validated input into pages on `image/formative/formative_1.jpg` to
`formative_6.jpg`: the cover, the results page (rank signal number, scores, score bars on
`2p/score_chart.jpg`, comment), the test information page (question count, counts and share of the
four levels, question counts of the first four units), the achievement page (unit graph
`4p/03 graph.jpg` with the score and average markers, the unit list, a bar for each level), the
per-session page that carries no field of the input, then the question table on `formative_6.jpg`,
one page for each 25 questions (15 rows in the left table, 10 in the right one), the first with the
signal picture `6p/signal_<rank_signal>.jpg`. Every box is in template pixels (1 px = 0.24 pt).
The cover, the test information, the achievement and the question pages are shared with `ganga`
(reports-batch2.md), which passes its own templates and overlays.
"""

from ..course_book import PagePlan, Plan
from ..geometry import px
from . import common
from .common import add, text
from .diag import COVER_DATE, COVER_STUDENT

REPORT_TYPE = "formative"
TEMPLATE_DIR = "image/formative/"
SCORE_CHART = TEMPLATE_DIR + "2p/score_chart.jpg"
GRAPH = TEMPLATE_DIR + "4p/03 graph.jpg"
AVERAGE_POINT = TEMPLATE_DIR + "4p/03 average-point.png"
ONESELF_POINT = TEMPLATE_DIR + "4p/03 oneself-point.png"

MY_COLOR = (0.12, 0.48, 0.77)
AVG_COLOR = (0.78, 0.78, 0.78)
LEVEL_COLORS = {"top": (0.12, 0.45, 0.75), "high": (0.2, 0.6, 0.8), "mid": (0.45, 0.72, 0.8), "low": (0.68, 0.86, 0.9)}

COVER_TITLE = (779.0, 1286.0, 2258.0, 1400.0)
# Results page.
RANK_BOX = (376.0, 525.0, 709.0, 753.0)
MY_SCORE = (1330.0, 639.0, 1654.0, 744.0)
AVG_SCORE = (1330.0, 1033.0, 1654.0, 1138.0)
SCORE_CHART_AT = (212.0, 1435.0)
BAR_ZERO_PX, BAR_PX_PER_POINT = 309.5, 18.545
BAR_ROWS = ((1671.0, 1733.0), (1804.0, 1866.0))
COMMENT_BOX = (945.0, 2538.0, 2240.0, 3238.0)
# Test information page.
TOTAL_BOX = (1330.0, 472.0, 2091.0, 578.0)
COUNT_X = (1575.0, 2156.0)
COUNT_ROWS = ((2581.0, 2660.0), (2769.0, 2848.0), (2958.0, 3036.0), (3145.0, 3224.0))
SHARE_PIE = (787.5, 2887.5, 332.5, 0.0)
UNIT_BUBBLES = ((735.0, 1204.0), (1795.5, 1204.0), (735.0, 1811.0), (1795.5, 1811.0))
# Achievement page: the graph's top left, and in graph pixels its centre, the radius of score 100 and
# the axes of the first four units (top, right, bottom, left).
GRAPH_AT = (236.0, 520.0)
GRAPH_CENTER = (805.0, 632.0)
GRAPH_RADIUS = 483.0
GRAPH_AXES = ((0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0))
UNIT_LIST = (1636.0, 1220.0, 2236.0, 1840.0)  # below the right-hand axis label of the graph
UNIT_COLUMNS = {"name": (1636.0, 1990.0), "my_score": (1990.0, 2113.0), "average": (2113.0, 2236.0)}
UNIT_ROW_MAX_PX = 72.0
LEVEL_CHART_X = (409.5, 1365.0)
LEVEL_ZERO_Y, LEVEL_PX_PER_POINT = 3250.0, 7.42
LEVEL_BAR_W = 120.0
# Question tables (`formative_6.jpg`).
SIGNAL_AT = (1798.0, 140.0)
QUESTION_TOP_PX, QUESTION_STEP_PX = 1794.0, 98.0
LEFT_ROWS = 15
RIGHT_OFFSET_PX = 1104.0
QUESTION_COLUMNS = {"no": (212.0, 394.0), "difficulty": (399.0, 592.0), "correct_rate": (822.0, 998.0)}
QUESTION_MARK = (1004.0, 1157.0)
QUESTION_SIZE = 10.5


def template(n):
    return TEMPLATE_DIR + "formative_%d.jpg" % n


def signal_rel(rank_signal):
    return TEMPLATE_DIR + "6p/signal_%d.jpg" % rank_signal


def cover(ctx, data, tpl=template):
    page = PagePlan(tpl(1))
    add(page, text(ctx, "p1.title", data["title"], COVER_TITLE, "l", 18))
    add(page, text(ctx, "p1.test_date", data["test_date"], COVER_DATE, "l", 14))
    add(page, text(ctx, "p1.student", data["student"], COVER_STUDENT, "l", 14))
    return page


def _results(ctx, data):
    page = PagePlan(template(2))
    add(page, text(ctx, "p2.rank_signal", str(data["rank_signal"]), RANK_BOX, "c", 28))
    add(page, text(ctx, "p2.my_score", common.number(data["my_score"]), MY_SCORE, "r", 24))
    add(page, text(ctx, "p2.avg_score", common.number(data["avg_score"]), AVG_SCORE, "r", 24))
    page.images.append(common.image_at(ctx, SCORE_CHART, *SCORE_CHART_AT))
    for (y0, y1), score, color in zip(BAR_ROWS, (data["my_score"], data["avg_score"]), (MY_COLOR, AVG_COLOR)):
        common.bar(page, BAR_ZERO_PX, BAR_ZERO_PX + BAR_PX_PER_POINT * score, y0, y1, color)
    page.fields.extend(common.comment_fields(ctx, "p2.comment", data["comment"], COMMENT_BOX))
    return page


def unit_counts(data):
    """The number of questions of each unit, by the question's `unit` equal to the unit's name."""
    return [sum(1 for q in data["questions"] if q["unit"] == u["name"]) for u in data["units"]]


def information(ctx, data, tpl=template):
    page = PagePlan(tpl(3))
    diff = data["difficulty"]
    total = sum(diff[lv]["questions"] for lv in common.LEVELS)
    add(page, text(ctx, "p3.questions", str(total), TOTAL_BOX, "r", 20))
    for k, level in enumerate(common.LEVELS):
        y0, y1 = COUNT_ROWS[k]
        add(page, text(ctx, "p3.%s.questions" % level, str(diff[level]["questions"]), (COUNT_X[0], y0, COUNT_X[1], y1), "r", 16))
    common.share_rings(page, *SHARE_PIE, diff, LEVEL_COLORS)
    counts = unit_counts(data)
    for i, (cx, cy) in enumerate(UNIT_BUBBLES[: len(data["units"])]):
        add(page, text(ctx, "p3.u%d.questions" % i, str(counts[i]), (cx - 170, cy - 52, cx + 170, cy + 61), "c", 20))
    return page


def achievement(ctx, data, tpl=template, graph=GRAPH, average_point=AVERAGE_POINT, oneself_point=ONESELF_POINT):
    page = PagePlan(tpl(4))
    gx, gy = GRAPH_AT
    page.images.append(common.image_at(ctx, graph, gx, gy))
    for i, unit in enumerate(data["units"][: len(GRAPH_AXES)]):
        dx, dy = GRAPH_AXES[i]
        for rel, score in ((average_point, unit["average"]), (oneself_point, unit["my_score"])):
            r = GRAPH_RADIUS * score / 100.0
            page.images.append(common.marker(ctx, rel, gx + GRAPH_CENTER[0] + dx * r, gy + GRAPH_CENTER[1] + dy * r))
    x0, top, x1, bottom = UNIT_LIST
    step = min(UNIT_ROW_MAX_PX, (bottom - top) / len(data["units"]))
    size = min(9.0, round(px(step) * 0.7, 1))
    for i, unit in enumerate(data["units"]):
        y0 = top + step * i
        values = {"name": unit["name"], "my_score": common.number(unit["my_score"]), "average": common.number(unit["average"])}
        for name, value in values.items():
            c0, c1 = UNIT_COLUMNS[name]
            add(page, text(ctx, "p4.u%d.%s" % (i, name), value, (c0, y0, c1, y0 + step), "l" if name == "name" else "r", size))
    slot = (LEVEL_CHART_X[1] - LEVEL_CHART_X[0]) / len(common.LEVELS)
    for k, level in enumerate(common.LEVELS):
        score = data["difficulty"][level]["score"]
        cx = LEVEL_CHART_X[0] + slot * k + slot / 2
        y = LEVEL_ZERO_Y - LEVEL_PX_PER_POINT * score
        common.bar(page, cx - LEVEL_BAR_W / 2, cx + LEVEL_BAR_W / 2, y, LEVEL_ZERO_Y, LEVEL_COLORS[level])
        add(page, text(ctx, "p4.%s.score" % level, common.number(score), (cx - slot / 2, y - 90, cx + slot / 2, y - 10), "c", 14))
    return page


def question_pages(ctx, data, tpl=template, signal=signal_rel):
    pages = []
    for n, chunk in enumerate(common.question_chunks(data["questions"])):
        page = PagePlan(tpl(6))
        if n == 0:
            page.images.append(common.image_at(ctx, signal(data["rank_signal"]), *SIGNAL_AT))
        prefix = "p%d" % (6 + n)
        for i, q in enumerate(chunk):
            index = n * common.QUESTIONS_PER_PAGE + i
            dx = 0.0 if i < LEFT_ROWS else RIGHT_OFFSET_PX
            y0 = QUESTION_TOP_PX + QUESTION_STEP_PX * (i % LEFT_ROWS)
            values = {"no": str(q["no"]), "difficulty": q["difficulty"], "correct_rate": common.number(q["correct_rate"])}
            for name, value in values.items():
                c0, c1 = QUESTION_COLUMNS[name]
                add(page, text(ctx, "%s.q%d.%s" % (prefix, index, name), value, (c0 + dx, y0, c1 + dx, y0 + QUESTION_STEP_PX), "c", QUESTION_SIZE))
            mark = (QUESTION_MARK[0] + dx, QUESTION_MARK[1] + dx)
            page.images.append(common.mark_image(ctx, q["correct"], mark, y0, QUESTION_STEP_PX))
        pages.append(page)
    return pages


def plan_formative(data, ctx):
    pages = [cover(ctx, data), _results(ctx, data), information(ctx, data), achievement(ctx, data), PagePlan(template(5))]
    return Plan(pages + question_pages(ctx, data))


def template_sequence(question_count):
    pages = -(-question_count // common.QUESTIONS_PER_PAGE)
    return [template(n) for n in range(1, 6)] + [template(6)] * pages


def required_formative_assets(data):
    """Relative paths of every asset a plan for `data` reads."""
    rels = {template(n) for n in range(1, 7)}
    rels.update({SCORE_CHART, GRAPH, AVERAGE_POINT, ONESELF_POINT, signal_rel(data["rank_signal"]), common.MARK_O, common.MARK_X})
    return sorted(rels)
