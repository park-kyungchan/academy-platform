"""The asset manifest and the verified asset bundle (specs/studyq-linux/core.md R9, R10)."""

import hashlib
import json
import os
import re
import stat

from .errors import StudyqError

SCHEMA_VERSION = "studyq-assets/1"
PINNED_REPORT_ZIP_SHA256 = "0578b55567ccb977a9857a7a93287a6fd130abf64ef533e61b00ab63b82ea7b3"
PINNED_FONT_SHA256 = "76f45ef4a6bcff344c837c95a7dcc26e017e38b5846d5ae0cdcb5b86be2e2d31"
FONT_REL = "fonts/NanumGothic-Regular.ttf"
MANIFEST_NAME = "manifest.json"
MANIFEST_MEMBERS = frozenset({"schemaVersion", "reportZipSha256", "fontSha256", "fileCount", "files"})
MAX_MANIFEST_BYTES = 4 * 1024 * 1024
_HEX64 = re.compile(r"[0-9a-f]{64}")


def sha256_hex(data):
    return hashlib.sha256(data).hexdigest()


class AssetBundle:
    """Read access to verified asset bytes, by relative path such as `image/mark_o.jpg`.

    A bundle made from a directory hashes every read again against the manifest entry, so a
    file changed after the verification is refused. A bundle made from a mapping holds bytes
    that the caller has already verified (the seam of the unit tests).
    """

    def __init__(self, loader, names):
        self._loader = loader
        self._names = frozenset(names)

    @classmethod
    def from_mapping(cls, mapping):
        data = dict(mapping)
        return cls(lambda rel: data[rel], data.keys())

    def has(self, rel):
        return rel in self._names

    def read(self, rel):
        if rel not in self._names:
            raise StudyqError("asset_missing", "not in the manifest: " + rel)
        return self._loader(rel)


def _reject_duplicates(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError("duplicate")
        out[key] = value
    return out


def _bad_constant(_name):
    raise ValueError("constant")


def _read_regular(path):
    """Read a regular file without following a final symbolic link; None when it is absent."""
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    except FileNotFoundError:
        return None
    except OSError:
        raise StudyqError("asset_missing", "not a readable regular file") from None
    with os.fdopen(fd, "rb") as handle:
        if not stat.S_ISREG(os.fstat(handle.fileno()).st_mode):
            raise StudyqError("asset_missing", "not a regular file")
        return handle.read()


def _safe_rel(rel):
    if not isinstance(rel, str) or not rel or "\\" in rel or "\x00" in rel:
        return False
    parts = rel.split("/")
    if rel.startswith("/") or any(p in ("", ".", "..") for p in parts):
        return False
    if rel == FONT_REL:
        return True
    return len(parts) >= 2 and parts[0] == "image"


def parse_manifest(raw):
    """Check the manifest bytes (R9) and return the `files` map."""
    try:
        doc = json.loads(raw.decode("utf-8"), object_pairs_hook=_reject_duplicates, parse_constant=_bad_constant)
    except (ValueError, RecursionError):
        raise StudyqError("asset_manifest_invalid", "manifest is not one JSON object") from None
    if not isinstance(doc, dict) or set(doc) != MANIFEST_MEMBERS:
        raise StudyqError("asset_manifest_invalid", "manifest members differ from the Interface")
    if doc["schemaVersion"] != SCHEMA_VERSION:
        raise StudyqError("asset_manifest_invalid", "schemaVersion")
    for member in ("reportZipSha256", "fontSha256"):
        if not isinstance(doc[member], str) or not _HEX64.fullmatch(doc[member]):
            raise StudyqError("asset_manifest_invalid", member)
    files = doc["files"]
    count = doc["fileCount"]
    if not isinstance(files, dict) or type(count) is not int or count != len(files):
        raise StudyqError("asset_manifest_invalid", "fileCount or files")
    for rel, digest in files.items():
        if not _safe_rel(rel):
            raise StudyqError("asset_manifest_invalid", "files path")
        if not isinstance(digest, str) or not _HEX64.fullmatch(digest):
            raise StudyqError("asset_manifest_invalid", "files digest")
    if doc["reportZipSha256"] != PINNED_REPORT_ZIP_SHA256:
        raise StudyqError("asset_hash_mismatch", "reportZipSha256")
    if doc["fontSha256"] != PINNED_FONT_SHA256:
        raise StudyqError("asset_hash_mismatch", "fontSha256")
    if FONT_REL not in files:
        raise StudyqError("asset_manifest_invalid", "font entry")
    if files[FONT_REL] != doc["fontSha256"]:
        raise StudyqError("asset_hash_mismatch", "font entry")
    return files


def verify_assets_dir(directory):
    """Verify `<directory>/manifest.json` and every file it names; return an AssetBundle (R9)."""
    root = os.path.realpath(directory)
    raw = None
    try:
        fd = os.open(os.path.join(root, MANIFEST_NAME), os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    except OSError:
        raise StudyqError("asset_manifest_invalid", "manifest absent") from None
    with os.fdopen(fd, "rb") as handle:
        if not stat.S_ISREG(os.fstat(handle.fileno()).st_mode):
            raise StudyqError("asset_manifest_invalid", "manifest is not a regular file")
        raw = handle.read(MAX_MANIFEST_BYTES + 1)
    if len(raw) > MAX_MANIFEST_BYTES:
        raise StudyqError("asset_manifest_invalid", "manifest too large")
    files = parse_manifest(raw)

    def locate(rel):
        parent = os.path.realpath(os.path.join(root, os.path.dirname(rel)))
        if parent != root and not parent.startswith(root + os.sep):
            raise StudyqError("asset_manifest_invalid", "files path")
        return os.path.join(parent, os.path.basename(rel))

    for rel in sorted(files):
        data = _read_regular(locate(rel))
        if data is None:
            raise StudyqError("asset_missing", "a file of the manifest is absent")
        if sha256_hex(data) != files[rel]:
            raise StudyqError("asset_hash_mismatch", "a file differs from its entry")

    def load(rel):
        data = _read_regular(locate(rel))
        if data is None:
            raise StudyqError("asset_missing", "a file of the manifest is absent")
        if sha256_hex(data) != files[rel]:
            raise StudyqError("asset_hash_mismatch", "a file differs from its entry")
        return data

    return AssetBundle(load, files.keys())
