"""The parity check of a `course_book` or `cism` PDF against its input (specs/studyq-linux/core.md R17).

    python3 -m studyq_linux.parity --pdf <pdf> --assets <dir> [--type course_book|cism]   < input.json

`--type` defaults to `course_book`.

It rebuilds the expected field table from the input, extracts the text of every page of the
PDF with its position and counts a mismatch for each field whose text is absent from its page
or lies outside its box by more than 2 pt, and for each word on a page that no field explains.
It also counts raw `rn` tokens. It prints counts and field ids only, never a value.
"""

import json
import sys

from .cli import read_input
from .course_book import plan_course_book
from .errors import StudyqError, clip_detail
from .manifest import verify_assets_dir
from .render import load_pymupdf, make_context
from .renderers.cism import plan_cism
from .schema import validate_cism, validate_course_book

TOLERANCE_PT = 2.0


def _inside(rect, box, tol):
    return rect.x0 >= box[0] - tol and rect.y0 >= box[1] - tol and rect.x1 <= box[2] + tol and rect.y1 <= box[3] + tol


def check_parity(pymupdf, pdf_bytes, plan, tolerance=TOLERANCE_PT):
    """Return {"mismatches", "rawRn", "fields", "pages", "failed"} (failed: field ids)."""
    doc = pymupdf.open(stream=pdf_bytes, filetype="pdf")
    failed = []
    raw_rn = 0
    try:
        if doc.page_count != len(plan.pages):
            failed.append("pages")
        for number, pp in enumerate(plan.pages[: doc.page_count]):
            page = doc[number]
            claimed = []
            for f in pp.fields:
                found = False
                for line in f.text.split("\n"):
                    hits = [r for r in page.search_for(line) if _inside(r, f.box, tolerance) and r not in claimed]
                    if not hits:
                        failed.append(f.id)
                        found = None
                        break
                    claimed.append(hits[0])
                    found = True
                if found is None:
                    continue
            words = page.get_text("words")
            for word in words:
                rect = pymupdf.Rect(word[:4])
                center = pymupdf.Point((rect.x0 + rect.x1) / 2, (rect.y0 + rect.y1) / 2)
                if word[4] == "rn":
                    raw_rn += 1
                if not any(center in c for c in claimed):
                    failed.append("p%d.unexplained" % (number + 1))
        return {
            "mismatches": len(failed),
            "rawRn": raw_rn,
            "fields": sum(len(p.fields) for p in plan.pages),
            "pages": doc.page_count,
            "failed": failed,
        }
    finally:
        doc.close()


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    try:
        names = args[0::2]
        if len(args) % 2 or len(set(names)) != len(names) or not {"--assets", "--pdf"} <= set(names) <= {"--assets", "--pdf", "--type"}:
            raise StudyqError("usage_invalid", "usage: --pdf <pdf> --assets <dir> [--type course_book|cism]")
        flags = dict(zip(names, args[1::2]))
        report_type = flags.get("--type", "course_book")
        if report_type not in ("course_book", "cism"):
            raise StudyqError("usage_invalid", "type is not course_book or cism")
        validate, planner = (validate_cism, plan_cism) if report_type == "cism" else (validate_course_book, plan_course_book)
        data = validate(read_input(sys.stdin.buffer))
        bundle = verify_assets_dir(flags["--assets"])
        pymupdf = load_pymupdf()
        plan = planner(data, make_context(pymupdf, bundle))
        with open(flags["--pdf"], "rb") as handle:
            result = check_parity(pymupdf, handle.read(), plan)
    except StudyqError as err:
        print(json.dumps({"status": "error", "code": err.code, "detail": clip_detail(err.detail)}, separators=(",", ":")))
        return 2 if err.code == "usage_invalid" else 1
    except OSError:
        print(json.dumps({"status": "error", "code": "render_failed", "detail": "pdf unreadable"}, separators=(",", ":")))
        return 1
    result["failed"] = sorted(set(result["failed"]))[:50]
    result["status"] = "ok" if not result["mismatches"] and not result["rawRn"] else "mismatch"
    print(json.dumps(result, separators=(",", ":")))
    return 0 if result["status"] == "ok" else 1


if __name__ == "__main__":
    sys.exit(main())
