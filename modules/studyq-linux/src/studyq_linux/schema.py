"""Validation of the `course_book` input (specs/studyq-linux/core.md R4).

Every refusal detail names a member path of this schema (`bunit_list[1].questions[0].no`),
never a value or an unknown key of the input (R7).
"""

import math

from .errors import StudyqError
from .textutil import single_line, split_lines

MAX_CHAPTERS = 30
MAX_QUESTIONS = 200
MAX_TESTS = 8
MAX_MUNITS = 11
LEVELS = ("top", "high", "mid", "low")


def _bad(kind, path):
    raise StudyqError("input_invalid", "%s:%s" % (kind, path))


def _obj(value, members, path):
    if not isinstance(value, dict):
        _bad("type", path)
    for name in members:
        if name not in value:
            _bad("missing", "%s.%s" % (path, name) if path else name)
    if set(value) - set(members):
        _bad("unknown_member", path or "input")
    return value


def _num(value, path, lo=0.0, hi=100.0):
    if type(value) not in (int, float):
        _bad("type", path)
    if isinstance(value, float) and not math.isfinite(value):
        _bad("range", path)
    if not lo <= value <= hi:
        _bad("range", path)
    return value


def _int(value, path, lo=0, hi=1_000_000):
    if type(value) is not int:
        _bad("type", path)
    if not lo <= value <= hi:
        _bad("range", path)
    return value


def _str(value, path, maxlen=200, multiline=False, required=True):
    if not isinstance(value, str):
        _bad("type", path)
    if "\x00" in value or len(value) > maxlen * 4:
        _bad("range", path)
    if multiline:
        lines = split_lines(value)
        text = "\n".join(lines).strip("\n")
    else:
        text = single_line(value)
    if any(ord(c) < 32 and c != "\n" for c in text):
        _bad("range", path)
    if required and not text:
        _bad("range", path)
    if len(text) > maxlen:
        _bad("range", path)
    return text


def _list(value, path, lo, hi):
    if not isinstance(value, list):
        _bad("type", path)
    if not lo <= len(value) <= hi:
        _bad("range", path)
    return value


def validate_course_book(data):
    """Return a normalised copy of the `course_book` input, or raise `input_invalid`."""
    top = _obj(
        data,
        (
            "period",
            "student",
            "grade",
            "my_average",
            "all_average",
            "comment",
            "score_range",
            "test_list",
            "bunit_list",
            "difficulty",
            "munit_list",
        ),
        "",
    )
    out = {
        "period": _str(top["period"], "period", 64),
        "student": _str(top["student"], "student", 64),
        "grade": _int(top["grade"], "grade", 1, 9),
        "my_average": _num(top["my_average"], "my_average"),
        "all_average": _num(top["all_average"], "all_average"),
        "comment": _str(top["comment"], "comment", 3000, multiline=True, required=False),
    }
    rng = _obj(top["score_range"], ("my", "average"), "score_range")
    out["score_range"] = {k: _num(rng[k], "score_range." + k) for k in ("my", "average")}

    tests = []
    for i, item in enumerate(_list(top["test_list"], "test_list", 1, MAX_TESTS)):
        path = "test_list[%d]" % i
        t = _obj(item, ("no", "title", "questions", "average", "score", "date"), path)
        tests.append(
            {
                "no": _int(t["no"], path + ".no"),
                "title": _str(t["title"], path + ".title"),
                "questions": _int(t["questions"], path + ".questions"),
                "average": _num(t["average"], path + ".average"),
                "score": _num(t["score"], path + ".score"),
                "date": _str(t["date"], path + ".date", 32),
            }
        )
    out["test_list"] = tests

    chapters = []
    for i, item in enumerate(_list(top["bunit_list"], "bunit_list", 1, MAX_CHAPTERS)):
        path = "bunit_list[%d]" % i
        c = _obj(item, ("name", "my_score", "average", "questions"), path)
        questions = []
        for j, q in enumerate(_list(c["questions"], path + ".questions", 1, MAX_QUESTIONS)):
            qp = "%s.questions[%d]" % (path, j)
            r = _obj(q, ("no", "type", "question_type", "difficulty", "correct_rate", "correct"), qp)
            if type(r["correct"]) is not bool:
                _bad("type", qp + ".correct")
            questions.append(
                {
                    "no": _int(r["no"], qp + ".no"),
                    "type": _str(r["type"], qp + ".type"),
                    "question_type": _str(r["question_type"], qp + ".question_type", 64),
                    "difficulty": _str(r["difficulty"], qp + ".difficulty", 32),
                    "correct_rate": _num(r["correct_rate"], qp + ".correct_rate"),
                    "correct": r["correct"],
                }
            )
        chapters.append(
            {
                "name": _str(c["name"], path + ".name"),
                "my_score": _num(c["my_score"], path + ".my_score"),
                "average": _num(c["average"], path + ".average"),
                "questions": questions,
            }
        )
    out["bunit_list"] = chapters

    diff = _obj(top["difficulty"], LEVELS, "difficulty")
    levels = {}
    for level in LEVELS:
        path = "difficulty." + level
        d = _obj(diff[level], ("questions", "score"), path)
        levels[level] = {
            "questions": _int(d["questions"], path + ".questions"),
            "score": _num(d["score"], path + ".score"),
        }
    out["difficulty"] = levels

    munits = []
    for i, item in enumerate(_list(top["munit_list"], "munit_list", 1, MAX_MUNITS)):
        path = "munit_list[%d]" % i
        m = _obj(item, ("name", "correct", "total", "average"), path)
        total = _int(m["total"], path + ".total", 1)
        munits.append(
            {
                "name": _str(m["name"], path + ".name"),
                "correct": _int(m["correct"], path + ".correct", 0, total),
                "total": total,
                "average": _num(m["average"], path + ".average"),
            }
        )
    out["munit_list"] = munits
    return out
