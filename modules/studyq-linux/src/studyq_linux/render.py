"""Drawing of a plan with PyMuPDF (specs/studyq-linux/core.md R8, R12, R13).

`render_course_book` is the entry point for an already-verified asset bundle; the command line
reaches it only through `manifest.verify_assets_dir`.
"""

from .course_book import COLOR_TEXT, PAD_PT, plan_course_book, required_assets
from .errors import StudyqError
from .geometry import PAGE_H, PAGE_W
from .imageinfo import image_dimensions
from .manifest import FONT_REL
from .renderers.cism import plan_cism, required_cism_assets
from .schema import validate_cism, validate_course_book

FONT_NAME = "nanum"
ASCENT = 0.856  # NanumGothic: top of the text box above the baseline, in em (the box is 1 em tall)


def load_pymupdf():
    try:
        import pymupdf
    except ImportError:
        raise StudyqError("render_failed", "pymupdf_unavailable") from None
    return pymupdf


def make_context(pymupdf, bundle):
    from .course_book import PlanContext

    font = pymupdf.Font(fontbuffer=bundle.read(FONT_REL))
    sizes = {}

    def image_size(rel):
        if rel not in sizes:
            dims = image_dimensions(bundle.read(rel))
            if dims is None:
                raise StudyqError("render_failed", "image unreadable")
            sizes[rel] = dims
        return sizes[rel]

    return PlanContext(lambda text, size: font.text_length(text, fontsize=size), image_size)


def draw_plan(pymupdf, plan, bundle):
    font_bytes = bundle.read(FONT_REL)
    font = pymupdf.Font(fontbuffer=font_bytes)
    doc = pymupdf.open()
    for pp in plan.pages:
        page = doc.new_page(width=PAGE_W, height=PAGE_H)
        page.insert_image(page.rect, stream=bundle.read(pp.template), keep_proportion=False)
        for image in pp.images:
            page.insert_image(pymupdf.Rect(*image.rect), stream=bundle.read(image.rel), keep_proportion=False)
        for shape in pp.shapes:
            if shape.kind == "frame":
                page.draw_rect(pymupdf.Rect(*shape.geometry), color=shape.fill, fill=None, width=0.75)
            elif shape.kind == "rect":
                page.draw_rect(pymupdf.Rect(*shape.geometry), color=None, fill=shape.fill)
            else:
                page.draw_polyline([pymupdf.Point(*p) for p in shape.geometry], color=None, fill=shape.fill, closePath=True)
        page.insert_font(fontname=FONT_NAME, fontbuffer=font_bytes)
        for f in pp.fields:
            width = font.text_length(f.text, fontsize=f.size)
            x0, y0, x1, y1 = f.box
            if f.align == "l":
                x = x0 + PAD_PT
            elif f.align == "r":
                x = x1 - PAD_PT - width
            else:
                x = (x0 + x1) / 2 - width / 2
            baseline = (y0 + y1) / 2 + (ASCENT - 0.5) * f.size
            page.insert_text(pymupdf.Point(x, baseline), f.text, fontsize=f.size, fontname=FONT_NAME, color=COLOR_TEXT)
    doc.set_metadata({})
    doc.del_xml_metadata()
    try:
        return doc.tobytes(garbage=4, deflate=True, no_new_id=True)
    finally:
        doc.close()


def render_course_book(data, bundle):
    """PDF bytes of the `course_book` report for an input and an already-verified bundle."""
    pymupdf = load_pymupdf()
    data = validate_course_book(data)
    for rel in required_assets(data):
        if not bundle.has(rel):
            raise StudyqError("asset_missing", "a template of the report is not in the manifest")
    plan = plan_course_book(data, make_context(pymupdf, bundle))
    return draw_plan(pymupdf, plan, bundle)


def render_cism(data, bundle):
    """PDF bytes of the `cism` report for an input and an already-verified bundle."""
    pymupdf = load_pymupdf()
    data = validate_cism(data)
    for rel in required_cism_assets(data):
        if not bundle.has(rel):
            raise StudyqError("asset_missing", "a template of the report is not in the manifest")
    plan = plan_cism(data, make_context(pymupdf, bundle))
    return draw_plan(pymupdf, plan, bundle)
