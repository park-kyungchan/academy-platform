"""The `smart_book` official SmartBook report (specs/studyq-linux/reports-batch2.md R1-R3, R6, R7).

The layout of `course_book` (specs/studyq-linux/core.md R13-R17, R29) on the templates
`image/smart_book/smartbook_1.jpg` to `smartbook_7.jpg`: 1, 2 (the grade picture `1p/lv0<grade>.jpg`),
3 (`5p/score_range_chart.jpg`, `5p/test_list_chart.jpg` with `5p/my_score_point.png` and
`5p/avg_score_point.png`), per chapter 4 (and 5 for each further 25 questions), 6, then 7
(`4p/munit_chart.jpg` with `4p/my_score_point.png`). The plan is the `course_book` plan with every
template and overlay read from `image/smart_book/`. `book_code` is validated and not printed: it is
the key of the LMS read.
"""

from .. import course_book
from ..course_book import ImageOp, PagePlan, Plan, PlanContext

REPORT_TYPE = "smart_book"
TEMPLATE_DIR = "image/smart_book/"
_SOURCE = course_book.TEMPLATE_DIR


def asset_rel(rel):
    """The SmartBook asset that stands for an asset of the `course_book` plan."""
    if not rel.startswith(_SOURCE):
        return rel  # the shared marks `image/mark_o.jpg`, `image/mark_x.jpg`
    rest = rel[len(_SOURCE) :]
    if rest.startswith("coursebook_"):
        return TEMPLATE_DIR + "smartbook_" + rest[len("coursebook_") :]
    if rest.startswith("1p/grade_"):
        return TEMPLATE_DIR + "1p/lv%02d.jpg" % int(rest[len("1p/grade_") : -len(".jpg")])
    return TEMPLATE_DIR + rest


def plan_smart_book(data, ctx):
    inner = PlanContext(ctx.measure, lambda rel: ctx.image_size(asset_rel(rel)))
    plan = course_book.plan_course_book(data, inner)
    pages = []
    for page in plan.pages:
        images = [ImageOp(asset_rel(i.rel), i.rect) for i in page.images]
        pages.append(PagePlan(asset_rel(page.template), images, list(page.shapes), list(page.fields)))
    return Plan(pages)


def template_sequence(chapter_row_counts):
    return [asset_rel(rel) for rel in course_book.template_sequence(chapter_row_counts)]


def required_smart_book_assets(data):
    """Relative paths of every asset a plan for `data` reads."""
    return sorted({asset_rel(rel) for rel in course_book.required_assets(data)})
