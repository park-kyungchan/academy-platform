"""`assemble-print`: print.zip archives to one deterministic PDF (specs/studyq-linux/core.md R20-R25).

Zero network: the module reads only local archive files under `archive_root` (R10). The TypeScript
server downloads the archives; this module never opens a connection. Every refusal detail names a
member path of the input or a fixed phrase, never a value of the input and never the archive key
(R7). The archive key is read from the private file that `STUDYQ_PRINT_KEY_FILE` names (R23).
"""

import hashlib
import os
import posixpath
import re
import stat
import zipfile
import zlib

from .errors import StudyqError
from .imageinfo import image_dimensions

KEY_ENV = "STUDYQ_PRINT_KEY_FILE"
DELIMITER = "|,|"
KINDS = ("exam", "examtext", "clinic")
SEL_TYPES = ("obj", "sbj", "des")
MAX_ITEMS = 200
MAX_EXAM_KEY = 999_999_999
MAX_EXAMTEXT_KEY = 999_999
MAX_LIST_CHARS = 65536
MAX_KEY_FILE_BYTES = 256
MAX_MEMBERS = 4096
MAX_MEMBER_BYTES = 64 * 1024 * 1024
MAX_ARCHIVE_BYTES = 256 * 1024 * 1024
CHUNK = 65536
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
PDF_MAGIC = b"%PDF-"
SYMLINK_MODE = 0o120000

# A4 in points, 36 pt margins (R8).
PAGE_W = 595.2756
PAGE_H = 841.8898
MARGIN = 36.0
FIT_WIDTH = 523.2756
BOTTOM = 805.8898
FIT_HEIGHT = 769.89

EXAM_TEMPLATE = "data/contents/exam/d6-%03d/d4-%02d/d2-%02d/ek-%d/%d-print.zip"
EXAMTEXT_TEMPLATE = "data/contents/examtext/d4-%02d/d2-%02d/etk-%d/%d-print.zip"
_KEY_ELEMENT = re.compile(r"^(0|[1-9][0-9]{0,8})$")
CLINIC_MEMBERS = ("clinic_list", "exam_key", "content_list", "clinic_type", "lecture_value")
CLINIC_OPTIONAL = ("sel_types", "sel_type", "include_answers", "include_explain")


def _bad(kind, path):
    raise StudyqError("input_invalid", "%s:%s" % (kind, path))


def exam_rel_path(key):
    """`data/contents/exam/...` for an integer key 0..999_999_999 (R21)."""
    if type(key) is not int or not 0 <= key <= MAX_EXAM_KEY:
        raise StudyqError("input_invalid", "range:lecture_key")
    d6, rest = divmod(key, 1_000_000)
    d4, rest = divmod(rest, 10_000)
    d2 = rest // 100
    return EXAM_TEMPLATE % (d6, d4, d2, key, key)


def examtext_rel_path(key):
    """`data/contents/examtext/...` for an integer key 0..999_999 (R21)."""
    if type(key) is not int or not 0 <= key <= MAX_EXAMTEXT_KEY:
        raise StudyqError("input_invalid", "range:examtext_key")
    d4, rest = divmod(key, 10_000)
    d2 = rest // 100
    return EXAMTEXT_TEMPLATE % (d4, d2, key, key)


def _members(value, required, optional, path):
    if not isinstance(value, dict):
        _bad("type", path)
    for name in required:
        if name not in value:
            _bad("missing", "%s.%s" % (path, name) if path != "input" else name)
    if set(value) - set(required) - set(optional):
        _bad("unknown_member", path)


def _flag(value, name, path):
    if name not in value:
        return False
    if type(value[name]) is not bool:
        _bad("type", "%s.%s" % (path, name) if path != "input" else name)
    return value[name]


def _sel(value, path):
    if type(value) is not str:
        _bad("type", path)
    if value not in SEL_TYPES:
        _bad("range", path)
    return value


def _joined(doc, name):
    value = doc[name]
    if type(value) is not str:
        _bad("type", name)
    if not value or len(value) > MAX_LIST_CHARS or "\x00" in value:
        _bad("range", name)
    elements = value.split(DELIMITER)
    if len(elements) > MAX_ITEMS:
        _bad("range", name)
    if any(not element for element in elements):
        _bad("empty_element", name)
    return elements


def validate_print_input(doc):
    """Return `(archive_root, kind, items)`; items are `(relative path, sel_type, answers, explain)`.

    Refuses with `input_invalid` naming a member path only (R4, R21, R22), before any file of the
    archives is opened; `archive_root` itself is checked here by lstat/realpath only.
    """
    if not isinstance(doc, dict):
        _bad("type", "input")
    root = doc.get("archive_root")
    if "archive_root" not in doc:
        _bad("missing", "archive_root")
    if type(root) is not str or not root or "\x00" in root or not os.path.isabs(root):
        _bad("type", "archive_root")
    kind = doc.get("kind")
    if "kind" not in doc:
        _bad("missing", "kind")
    if type(kind) is not str:
        _bad("type", "kind")
    if kind not in KINDS:
        _bad("range", "kind")
    if kind == "clinic":
        items = _clinic_items(doc)
    else:
        items = _exam_items(doc, kind)
    try:
        info = os.lstat(root)
        real = os.path.realpath(root)
    except OSError:
        _bad("range", "archive_root")
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode) or real != os.path.abspath(root):
        _bad("range", "archive_root")
    return root, kind, items


def _exam_items(doc, kind):
    key_name = "lecture_key" if kind == "exam" else "examtext_key"
    _members(doc, ("archive_root", "kind", "items"), (), "input")
    listed = doc["items"]
    if not isinstance(listed, list):
        _bad("type", "items")
    if not 1 <= len(listed) <= MAX_ITEMS:
        _bad("range", "items")
    items = []
    for index, item in enumerate(listed):
        path = "items[%d]" % index
        _members(item, (key_name, "sel_type"), ("include_answers", "include_explain"), path)
        key = item[key_name]
        if type(key) is not int:
            _bad("type", "%s.%s" % (path, key_name))
        limit = MAX_EXAM_KEY if kind == "exam" else MAX_EXAMTEXT_KEY
        if not 0 <= key <= limit:
            _bad("range", "%s.%s" % (path, key_name))
        rel = exam_rel_path(key) if kind == "exam" else examtext_rel_path(key)
        items.append((rel, _sel(item["sel_type"], path + ".sel_type"), _flag(item, "include_answers", path), _flag(item, "include_explain", path)))
    return items


def _clinic_items(doc):
    _members(doc, ("archive_root", "kind") + CLINIC_MEMBERS, CLINIC_OPTIONAL, "input")
    lists = {name: _joined(doc, name) for name in ("clinic_list", "content_list", "lecture_value", "exam_key")}
    clinic_type = doc["clinic_type"]
    if type(clinic_type) is not str:
        _bad("type", "clinic_type")
    if not clinic_type or len(clinic_type) > 200 or "\x00" in clinic_type:
        _bad("range", "clinic_type")
    count = len(lists["clinic_list"])
    for name in ("content_list", "lecture_value", "exam_key"):
        if len(lists[name]) != count:
            _bad("count_mismatch", name)
    keys = []
    for element in lists["exam_key"]:
        if not _KEY_ELEMENT.match(element):
            _bad("range", "exam_key")
        keys.append(int(element))
    if "sel_types" in doc and "sel_type" in doc:
        _bad("conflict", "sel_type")
    if "sel_types" in doc:
        sels = _joined(doc, "sel_types")
        if len(sels) != count:
            _bad("count_mismatch", "sel_types")
        for sel in sels:
            _sel(sel, "sel_types")
    else:
        sels = [_sel(doc.get("sel_type", "obj"), "sel_type")] * count
    answers = _flag(doc, "include_answers", "input")
    explain = _flag(doc, "include_explain", "input")
    return [(exam_rel_path(key), sel, answers, explain) for key, sel in zip(keys, sels)]


def read_key(environ):
    """The archive key bytes from the private file of `STUDYQ_PRINT_KEY_FILE` (R23), or `archive_key_missing`."""
    path = environ.get(KEY_ENV)
    if not path:
        raise StudyqError("archive_key_missing", "key file not configured")
    fd = None
    try:
        before = os.lstat(path)
        if not stat.S_ISREG(before.st_mode):
            raise StudyqError("archive_key_missing", "key file unqualified")
        fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0))
        info = os.fstat(fd)
        if (info.st_dev, info.st_ino) != (before.st_dev, before.st_ino) or not stat.S_ISREG(info.st_mode):
            raise StudyqError("archive_key_missing", "key file unqualified")
        if info.st_uid != os.geteuid() or (info.st_mode & 0o077) != 0 or not 1 <= info.st_size <= MAX_KEY_FILE_BYTES:
            raise StudyqError("archive_key_missing", "key file unqualified")
        raw = os.read(fd, MAX_KEY_FILE_BYTES + 1)
    except OSError:
        raise StudyqError("archive_key_missing", "key file unreadable") from None
    finally:
        if fd is not None:
            os.close(fd)
    key = raw.rstrip(b"\r\n") if len(raw) <= MAX_KEY_FILE_BYTES else b""
    if not key:
        raise StudyqError("archive_key_missing", "key file unqualified")
    return key


def _refused(detail):
    return StudyqError("archive_refused", detail)


def check_member_names(infos):
    """ZipSlip defence over every member before any is read (R24, R25)."""
    if len(infos) > MAX_MEMBERS:
        raise _refused("too many members")
    root = "/extract"
    for info in infos:
        name = info.filename
        if not name or "\x00" in name or name[0] in "/\\" or re.match(r"^[A-Za-z]:", name):
            raise _refused("unsafe member name")
        parts = name.replace("\\", "/").split("/")
        if ".." in parts:
            raise _refused("unsafe member name")
        resolved = posixpath.normpath(posixpath.join(root, name.replace("\\", "/")))
        if resolved != root and not resolved.startswith(root + "/"):
            raise _refused("unsafe member name")
        if (info.external_attr >> 16) & 0o170000 == SYMLINK_MODE:
            raise _refused("symbolic link member")


def _wanted(sel_type, answers, explain):
    names = {"print-%s-exam.png" % sel_type}
    if sel_type == "obj":
        names.add("print-obj-objective.png")
    if answers:
        names.add("print-%s-answer.png" % sel_type)
    if explain:
        names.add("print-%s-explain.png" % sel_type)
    return names


def _stream(archive, info, key, budget):
    """The decompressed bytes of one member, stopping at the first byte over a limit (R25)."""
    chunks = []
    size = 0
    with archive.open(info, "r", pwd=key) as handle:
        while True:
            chunk = handle.read(CHUNK)
            if not chunk:
                break
            size += len(chunk)
            budget[0] += len(chunk)
            if size > MAX_MEMBER_BYTES or budget[0] > MAX_ARCHIVE_BYTES:
                raise _refused("size limit")
            chunks.append(chunk)
    return b"".join(chunks)


def read_parts(path, key, sel_type, answers, explain):
    """The `("png"|"pdf", bytes)` parts of one archive in member order (R8, R20, R24, R25)."""
    try:
        archive = zipfile.ZipFile(path, "r")
    except (zipfile.BadZipFile, EOFError, zlib.error, NotImplementedError):
        raise _refused("archive unreadable") from None
    with archive:
        try:
            infos = archive.infolist()
            check_member_names(infos)
            wanted = _wanted(sel_type, answers, explain)
            pngs = [i for i in infos if not i.is_dir() and posixpath.basename(i.filename.replace("\\", "/")) in wanted]
            pdfs = [i for i in infos if not i.is_dir() and i.filename.lower().endswith(".pdf")]
            if pngs and pdfs:
                raise _refused("png and pdf members")
            if not pngs and not pdfs:
                raise _refused("no print member")
            parts = []
            budget = [0]
            for info in pngs or pdfs:
                data = _stream(archive, info, key, budget)
                if pngs:
                    if data[:8] != PNG_MAGIC or image_dimensions(data) is None:
                        raise _refused("member is not a PNG")
                    parts.append(("png", data))
                else:
                    if data[:5] != PDF_MAGIC:
                        raise _refused("member is not a PDF")
                    parts.append(("pdf", data))
            return parts
        except StudyqError:
            raise
        except (RuntimeError, zipfile.BadZipFile, zlib.error, NotImplementedError, EOFError):
            raise _refused("archive unreadable") from None


def collect_parts(root, items, key):
    """The parts of every archive in input order; a missing archive refuses before any is read (R20)."""
    root_real = os.path.realpath(root)
    paths = []
    for rel, _sel_type, _answers, _explain in items:
        path = os.path.join(root, rel)
        try:
            info = os.lstat(path)
        except FileNotFoundError:
            raise StudyqError("archive_missing", "archive not found") from None
        except OSError:
            raise _refused("archive not readable") from None
        if not stat.S_ISREG(info.st_mode) or not os.path.realpath(path).startswith(root_real + os.sep):
            raise _refused("archive is not a regular file under the root")
        paths.append(path)
    parts = []
    for path, (_rel, sel_type, answers, explain) in zip(paths, items):
        parts.extend(read_parts(path, key, sel_type, answers, explain))
    return parts


def layout_images(sizes):
    """Placements for PNGs of `(width, height)` pixels: a list of `(page index, x0, y0, x1, y1)` in points (R8).

    Images are stacked from the top margin on A4 pages at the fitting width, a taller one scaled to
    `FIT_HEIGHT`; the next page starts when an image would end below `BOTTOM`.
    """
    placed = []
    page = -1
    y = MARGIN
    for width, height in sizes:
        w = FIT_WIDTH
        h = FIT_WIDTH * height / width
        if h > FIT_HEIGHT:
            w, h = FIT_HEIGHT * width / height, FIT_HEIGHT
        if page < 0 or (y > MARGIN and y + h > BOTTOM):
            page += 1
            y = MARGIN
        placed.append((page, MARGIN, y, MARGIN + w, y + h))
        y += h
    return placed


def assemble_pdf(parts):
    """`(pdf bytes, page count)` for the parts, deterministic (no id, no time) (R8)."""
    from .render import load_pymupdf

    pymupdf = load_pymupdf()
    doc = pymupdf.open()
    try:
        run = []

        def flush():
            sizes = [image_dimensions(data) for data in run]
            pages = {}
            for data, (index, x0, y0, x1, y1) in zip(run, layout_images(sizes)):
                if index not in pages:
                    pages[index] = doc.new_page(width=PAGE_W, height=PAGE_H)
                pages[index].insert_image(pymupdf.Rect(x0, y0, x1, y1), stream=data, keep_proportion=False)
            del run[:]

        try:
            for kind, data in parts:
                if kind == "png":
                    run.append(data)
                    continue
                flush()
                source = pymupdf.open(stream=data, filetype="pdf")
                try:
                    doc.insert_pdf(source)
                finally:
                    source.close()
            flush()
        except StudyqError:
            raise
        except Exception:
            raise _refused("member cannot be drawn") from None
        doc.set_metadata({})
        doc.del_xml_metadata()
        pages = doc.page_count
        if pages < 1:
            raise _refused("no pages")
        return doc.tobytes(garbage=4, deflate=True, no_new_id=True), pages
    finally:
        doc.close()


def assemble_print(doc, environ):
    """Run `assemble-print` on a parsed input: `(pdf bytes, page count, report type, sha256)`."""
    root, kind, items = validate_print_input(doc)
    key = read_key(environ)
    parts = collect_parts(root, items, key)
    pdf, pages = assemble_pdf(parts)
    return pdf, pages, "print_" + kind, hashlib.sha256(pdf).hexdigest()
