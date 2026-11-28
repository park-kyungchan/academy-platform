"""The command line: `render` and `assemble-print` (specs/studyq-linux/core.md R1-R7, R20-R25).

One run, one PDF, one line on standard output. Refusals print
`{"status":"error","code":...,"detail":...}`; a detail never holds a value of the input.
"""

import hashlib
import json
import os
import signal
import sys
import tempfile

from .errors import StudyqError, clip_detail
from .manifest import verify_assets_dir
from .schema import (
    validate_cism,
    validate_course_book,
    validate_diag,
    validate_formative,
    validate_ganga,
    validate_na_score,
    validate_new_diag,
    validate_smart_book,
)

MAX_STDIN_BYTES = 4 * 1024 * 1024
KNOWN_TYPES = ("course_book", "cism", "diag", "new_diag", "formative", "ganga", "na_score", "smart_book")
VALIDATORS = {
    "course_book": validate_course_book,
    "cism": validate_cism,
    "diag": validate_diag,
    "new_diag": validate_new_diag,
    "formative": validate_formative,
    "ganga": validate_ganga,
    "na_score": validate_na_score,
    "smart_book": validate_smart_book,
}
FLAGS = {"render": ("--type", "--assets", "--out"), "assemble-print": ("--out",)}


def parse_words(argv):
    """The two forms of the Interface; anything else is `usage_invalid`. Returns (verb, flags)."""
    if not argv or argv[0] not in FLAGS:
        raise StudyqError("usage_invalid", "unknown verb")
    verb, allowed, flags = argv[0], FLAGS[argv[0]], {}
    rest = argv[1:]
    i = 0
    while i < len(rest):
        word = rest[i]
        if word not in allowed:
            raise StudyqError("usage_invalid", "positional word or unknown flag")
        if word in flags:
            raise StudyqError("usage_invalid", "repeated flag")
        if i + 1 >= len(rest) or not rest[i + 1] or rest[i + 1].startswith("--"):
            raise StudyqError("usage_invalid", "flag without a value")
        flags[word] = rest[i + 1]
        i += 2
    if set(flags) != set(allowed):
        raise StudyqError("usage_invalid", "missing flag")
    return verb, {k[2:]: v for k, v in flags.items()}


def _reject_duplicates(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError("duplicate")
        out[key] = value
    return out


def _bad_constant(_name):
    raise ValueError("constant")


def read_input(stream):
    """Standard input as one JSON object (R4)."""
    raw = stream.read(MAX_STDIN_BYTES + 1)
    if len(raw) > MAX_STDIN_BYTES:
        raise StudyqError("input_invalid", "larger than 4 MiB")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        raise StudyqError("input_invalid", "not UTF-8") from None
    try:
        doc = json.loads(text, object_pairs_hook=_reject_duplicates, parse_constant=_bad_constant)
    except (ValueError, RecursionError):
        raise StudyqError("input_invalid", "not one JSON document") from None
    if not isinstance(doc, dict):
        raise StudyqError("input_invalid", "not a JSON object")
    return doc


def write_atomic(out_path, data):
    """Write `data` beside `out_path` and link it into place; never clobber, always clean up (R6)."""
    directory = os.path.dirname(out_path)
    try:
        fd, tmp = tempfile.mkstemp(prefix=".studyq-", suffix=".tmp", dir=directory)
    except OSError:
        raise StudyqError("render_failed", "output directory not writable") from None
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(tmp, 0o644)
        try:
            os.link(tmp, out_path)
        except FileExistsError:
            raise StudyqError("output_exists", "output exists") from None
        except OSError:
            raise StudyqError("render_failed", "output not writable") from None
    finally:
        try:
            os.unlink(tmp)
        except FileNotFoundError:
            pass


def count_pages(pdf_bytes):
    from .render import load_pymupdf

    pymupdf = load_pymupdf()
    doc = pymupdf.open(stream=pdf_bytes, filetype="pdf")
    try:
        return doc.page_count
    finally:
        doc.close()


def run(argv, stdin):
    """Run one command; return (exit status, the one line for standard output)."""
    try:
        line = _execute(argv, stdin)
        return 0, line
    except StudyqError as err:
        status = 2 if err.code == "usage_invalid" else 1
        return status, _error_line(err.code, err.detail)
    except Exception as err:  # R2: any other failure is render_failed, with the class name only
        return 1, _error_line("render_failed", type(err).__name__)


def _error_line(code, detail):
    return json.dumps({"status": "error", "code": code, "detail": clip_detail(detail)}, ensure_ascii=False, separators=(",", ":"))


def _ok_line(report_type, pages, sha256, out_path):
    return json.dumps(
        {"status": "ok", "reportType": report_type, "pages": pages, "sha256": sha256, "outputPath": out_path},
        ensure_ascii=False,
        separators=(",", ":"),
    )


def _execute(argv, stdin):
    verb, flags = parse_words(argv)
    if verb == "assemble-print":
        return _assemble_print(flags, stdin)
    report_type = flags["type"]
    if report_type not in KNOWN_TYPES:
        raise StudyqError("type_unknown", "type is not one of the eight report types")
    out_path = os.path.abspath(flags["out"])
    if os.path.lexists(out_path):
        raise StudyqError("output_exists", "output exists")
    data = VALIDATORS[report_type](read_input(stdin))
    bundle = verify_assets_dir(flags["assets"])
    from .render import render_report

    pdf = render_report(report_type, data, bundle)
    pages = count_pages(pdf)
    write_atomic(out_path, pdf)
    return _ok_line(report_type, pages, hashlib.sha256(pdf).hexdigest(), out_path)


def _assemble_print(flags, stdin):
    """`assemble-print` (R20-R25): local archives only, the key from the private file of the environment (R23)."""
    out_path = os.path.abspath(flags["out"])
    if os.path.lexists(out_path):
        raise StudyqError("output_exists", "output exists")
    from .print_zip import assemble_print

    pdf, pages, report_type, sha256 = assemble_print(read_input(stdin), os.environ)
    write_atomic(out_path, pdf)
    return _ok_line(report_type, pages, sha256, out_path)


def main(argv=None):
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    args = list(sys.argv[1:] if argv is None else argv)
    status, line = run(args, sys.stdin.buffer)
    sys.stdout.write(line + "\n")
    sys.stdout.flush()
    return status
