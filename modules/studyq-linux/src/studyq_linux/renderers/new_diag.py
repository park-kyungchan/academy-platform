"""The `new_diag` official new-student diagnostic report (specs/studyq-linux/reports-batch1.md R1-R3, R5, R7).

The layout of `diag` on the templates `image/new_diag/diag_1.jpg` to `diag_10.jpg`, with the
overlays of `image/diag/`: pages 1 to 8 as `diag` (no signal light and no difficulty chart, which
the new-student templates do not use; the graph is `4p/03_graph.jpg`), the question table on
`diag_9.jpg` (one page for each 25 questions), then the learning-style page `diag_10.jpg` with the
picture `9p/<mbti>.jpg`. `apply_code` is validated and not printed: it is the key of the LMS read.
"""

from ..course_book import PagePlan, Plan
from . import common
from .diag import OVERLAY_DIR, Style, fixed_pages, question_pages, required_style_assets, template_sequence

REPORT_TYPE = "new_diag"
NEW_DIAG = Style(
    template_dir="image/new_diag/",
    graph=OVERLAY_DIR + "4p/03_graph.jpg",
    chart_rows=((1694.0, 1773.0), (1834.0, 1913.0)),
    my_color=(0.84, 0.22, 0.16),
    avg_color=(0.6, 0.47, 0.42),
    level_colors={"top": (0.96, 0.68, 0.0), "high": (0.42, 0.38, 0.35), "mid": (0.68, 0.6, 0.5), "low": (0.85, 0.8, 0.74)},
    lights=False,
    difficulty_chart=False,
)
STYLE_PAGE = NEW_DIAG.template(10)
MBTI_BOX = (227.0, 262.0, 2258.0, 718.0)


def mbti_rel(code):
    return OVERLAY_DIR + "9p/%s.jpg" % code


def plan_new_diag(data, ctx):
    pages = fixed_pages(ctx, NEW_DIAG, data) + question_pages(ctx, NEW_DIAG.template(9), data, 9)
    last = PagePlan(STYLE_PAGE)
    last.images.append(common.fit_image(ctx, mbti_rel(data["mbti"]), MBTI_BOX))
    return Plan(pages + [last])


def new_diag_template_sequence(question_count):
    return template_sequence(question_count, NEW_DIAG, (STYLE_PAGE,))


def required_new_diag_assets(data):
    """Relative paths of every asset a plan for `data` reads."""
    rels = required_style_assets(NEW_DIAG, data)
    rels.update({STYLE_PAGE, mbti_rel(data["mbti"])})
    return sorted(rels)
