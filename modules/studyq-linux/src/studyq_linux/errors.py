"""Error codes of the executor (specs/studyq-linux/core.md, "Error codes")."""

CODES = frozenset(
    {
        "usage_invalid",
        "input_invalid",
        "type_unknown",
        "output_exists",
        "asset_manifest_invalid",
        "asset_missing",
        "asset_hash_mismatch",
        "archive_key_missing",
        "archive_missing",
        "archive_refused",
        "not_implemented",
        "render_failed",
    }
)

DETAIL_MAX_BYTES = 256


def clip_detail(text):
    """Return `text` cut to at most 256 bytes of UTF-8 on a character boundary."""
    raw = str(text).encode("utf-8")[:DETAIL_MAX_BYTES]
    return raw.decode("utf-8", "ignore")


class StudyqError(Exception):
    """A refusal with one code of the table and a detail that holds no input value."""

    def __init__(self, code, detail):
        if code not in CODES:
            raise ValueError("unknown code")
        super().__init__(code)
        self.code = code
        self.detail = clip_detail(detail)
