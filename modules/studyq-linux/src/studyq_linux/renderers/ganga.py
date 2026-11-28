"""The `ganga` official Gang-A report (specs/studyq-linux/reports-batch2.md R1-R4, R7).

`plan_ganga` turns a validated input into pages on `image/ganga/ganga_1.jpg` to `ganga_6.jpg`: the
cover, the results page (the pyramid picture `1p/01 pyramid<pyramid_tier>.jpg`, the score and the
average, the comment), the test information page (question count, counts and share of the four
levels, question counts of the first four units), the achievement page (unit graph `3p/03 graph.jpg`
with the average and score markers, the unit list, a bar for each level), a page that carries no
field of the input, then the question table on `ganga_6.jpg`, one page for each 25 questions (15
rows in the left table, 10 in the right), the first with the signal picture
`5p/05 signal_0<rank_signal>.jpg`. The overlay folders count the pages from 0 (`1p` is page 2).
The cover, the information, the achievement and the question pages are those of `formative`
(reports-batch1.md) on the Gang-A templates. Every box is in template pixels (1 px = 0.24 pt).
"""

from ..course_book import PagePlan, Plan
from . import common, formative
from .common import add, text

REPORT_TYPE = "ganga"
TEMPLATE_DIR = "image/ganga/"
GRAPH = TEMPLATE_DIR + "3p/03 graph.jpg"
AVERAGE_POINT = TEMPLATE_DIR + "3p/03 average-point.png"
ONESELF_POINT = TEMPLATE_DIR + "3p/03 oneself-point.png"

# Results page: the pyramid picture fills the space of the score chart, never enlarged.
PYRAMID_BOX = (212.0, 1250.0, 2269.0, 2450.0)
MY_SCORE = formative.MY_SCORE
AVG_SCORE = formative.AVG_SCORE
COMMENT_BOX = formative.COMMENT_BOX


def template(n):
    return TEMPLATE_DIR + "ganga_%d.jpg" % n


def pyramid_rel(tier):
    return TEMPLATE_DIR + "1p/01 pyramid%d.jpg" % tier


def signal_rel(rank_signal):
    return TEMPLATE_DIR + "5p/05 signal_0%d.jpg" % rank_signal


def _results(ctx, data):
    page = PagePlan(template(2))
    page.images.append(common.fit_image(ctx, pyramid_rel(data["pyramid_tier"]), PYRAMID_BOX))
    add(page, text(ctx, "p2.my_score", common.number(data["my_score"]), MY_SCORE, "r", 24))
    add(page, text(ctx, "p2.avg_score", common.number(data["avg_score"]), AVG_SCORE, "r", 24))
    page.fields.extend(common.comment_fields(ctx, "p2.comment", data["comment"], COMMENT_BOX))
    return page


def plan_ganga(data, ctx):
    pages = [
        formative.cover(ctx, data, template),
        _results(ctx, data),
        formative.information(ctx, data, template),
        formative.achievement(ctx, data, template, GRAPH, AVERAGE_POINT, ONESELF_POINT),
        PagePlan(template(5)),
    ]
    return Plan(pages + formative.question_pages(ctx, data, template, signal_rel))


def template_sequence(question_count):
    pages = -(-question_count // common.QUESTIONS_PER_PAGE)
    return [template(n) for n in range(1, 6)] + [template(6)] * pages


def required_ganga_assets(data):
    """Relative paths of every asset a plan for `data` reads."""
    rels = {template(n) for n in range(1, 7)}
    rels.update({pyramid_rel(data["pyramid_tier"]), GRAPH, AVERAGE_POINT, ONESELF_POINT, signal_rel(data["rank_signal"]), common.MARK_O, common.MARK_X})
    return sorted(rels)
