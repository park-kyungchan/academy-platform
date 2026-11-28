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


MAX_CISM_ROWS = 1200
MAX_CISM_SECTIONS = 8
CISM_SLOTS = {4: (1, 2), 5: (1, 2, 3, 4), 6: (1, 2)}


def validate_cism(data):
    """Return a normalised copy of the `cism` input, or raise `input_invalid` (R19)."""
    top = _obj(data, ("week_no", "week_label", "teacher", "period", "chart_data", "report_list"), "")
    out = {
        "week_no": _int(top["week_no"], "week_no", 1, 53),
        "week_label": _str(top["week_label"], "week_label", 120),
        "teacher": _str(top["teacher"], "teacher", 64),
        "period": _str(top["period"], "period", 64),
    }
    rows = []
    for i, item in enumerate(_list(top["chart_data"], "chart_data", 0, MAX_CISM_ROWS)):
        path = "chart_data[%d]" % i
        r = _obj(item, ("group", "student", "class_mode", "think_note", "week_test"), path)
        rows.append(
            {
                "group": _str(r["group"], path + ".group", 120),
                "student": _str(r["student"], path + ".student", 64),
                "class_mode": _str(r["class_mode"], path + ".class_mode", 16, required=False),
                "think_note": _str(r["think_note"], path + ".think_note", 32, required=False),
                "week_test": _str(r["week_test"], path + ".week_test", 32, required=False),
            }
        )
    out["chart_data"] = rows
    sections, seen = [], set()
    for i, item in enumerate(_list(top["report_list"], "report_list", 1, MAX_CISM_SECTIONS)):
        path = "report_list[%d]" % i
        s = _obj(item, ("group", "slot", "title", "text"), path)
        group = _int(s["group"], path + ".group", 4, 6)
        slot = _int(s["slot"], path + ".slot", 1, max(CISM_SLOTS[group]))
        if (group, slot) in seen:
            _bad("duplicate", path + ".slot")
        seen.add((group, slot))
        sections.append(
            {
                "group": group,
                "slot": slot,
                "title": _str(s["title"], path + ".title", 120),
                "text": _str(s["text"], path + ".text", 40000, multiline=True, required=False),
            }
        )
    out["report_list"] = sections
    return out


# The official diagnostic, new-student diagnostic and formative reports (specs/studyq-linux/reports-batch1.md R4-R6).
MAX_REPORT_QUESTIONS = 100
MAX_DIAG_AREAS = 8
MAX_FORMATIVE_UNITS = 12
SIGNALS = ("green", "yellow", "red")
RATINGS = ("nice", "normal", "base", "fail")
MBTI_CODES = (
    "ENFJ", "ENFP", "ENTJ", "ENTP", "ESFJ", "ESFP", "ESTJ", "ESTP",
    "INFJ", "INFP", "INTJ", "INTP", "ISFJ", "ISFP", "ISTJ", "ISTP",
    "other",
)  # fmt: skip


def _enum(value, path, allowed):
    if not isinstance(value, str):
        _bad("type", path)
    if value not in allowed:
        _bad("range", path)
    return value


def _bool(value, path):
    if type(value) is not bool:
        _bad("type", path)
    return value


def _report_difficulty(value):
    diff = _obj(value, LEVELS, "difficulty")
    levels = {}
    for level in LEVELS:
        path = "difficulty." + level
        d = _obj(diff[level], ("questions", "score"), path)
        levels[level] = {
            "questions": _int(d["questions"], path + ".questions", 0, 500),
            "score": _num(d["score"], path + ".score"),
        }
    return levels


def _report_questions(value, group):
    """The question rows of a report; `group` is `area` (diag, new_diag) or `unit` (formative)."""
    rows = []
    for i, item in enumerate(_list(value, "questions", 1, MAX_REPORT_QUESTIONS)):
        path = "questions[%d]" % i
        q = _obj(item, ("no", group, "difficulty", "correct_rate", "correct"), path)
        rows.append(
            {
                "no": _int(q["no"], path + ".no", 1, 500),
                group: _str(q[group], path + "." + group, 80),
                "difficulty": _str(q["difficulty"], path + ".difficulty", 32),
                "correct_rate": _num(q["correct_rate"], path + ".correct_rate"),
                "correct": _bool(q["correct"], path + ".correct"),
            }
        )
    return rows


DIAG_MEMBERS = ("title", "student", "test_date", "grade", "total_score", "avg_score", "comment", "areas", "difficulty", "questions")


def _diag_common(top):
    out = {
        "title": _str(top["title"], "title", 120),
        "student": _str(top["student"], "student", 64),
        "test_date": _str(top["test_date"], "test_date", 32),
        "grade": _int(top["grade"], "grade", 1, 9),
        "total_score": _num(top["total_score"], "total_score"),
        "avg_score": _num(top["avg_score"], "avg_score"),
        "comment": _str(top["comment"], "comment", 3000, multiline=True, required=False),
    }
    areas = []
    for i, item in enumerate(_list(top["areas"], "areas", 1, MAX_DIAG_AREAS)):
        path = "areas[%d]" % i
        a = _obj(item, ("name", "score", "average", "signal", "rating"), path)
        areas.append(
            {
                "name": _str(a["name"], path + ".name", 80),
                "score": _num(a["score"], path + ".score"),
                "average": _num(a["average"], path + ".average"),
                "signal": _enum(a["signal"], path + ".signal", SIGNALS),
                "rating": _enum(a["rating"], path + ".rating", RATINGS),
            }
        )
    out["areas"] = areas
    out["difficulty"] = _report_difficulty(top["difficulty"])
    out["questions"] = _report_questions(top["questions"], "area")
    return out


def validate_diag(data):
    """Return a normalised copy of the `diag` input, or raise `input_invalid` (reports-batch1.md R4)."""
    return _diag_common(_obj(data, DIAG_MEMBERS, ""))


def validate_new_diag(data):
    """Return a normalised copy of the `new_diag` input, or raise `input_invalid` (reports-batch1.md R5)."""
    top = _obj(data, DIAG_MEMBERS + ("apply_code", "mbti"), "")
    out = _diag_common(top)
    out["apply_code"] = _str(top["apply_code"], "apply_code", 64)
    out["mbti"] = _enum(top["mbti"], "mbti", MBTI_CODES)
    return out


def validate_formative(data):
    """Return a normalised copy of the `formative` input, or raise `input_invalid` (reports-batch1.md R6)."""
    top = _obj(
        data,
        ("title", "student", "test_date", "my_score", "avg_score", "rank_signal", "comment", "units", "difficulty", "questions"),
        "",
    )
    out = {
        "title": _str(top["title"], "title", 120),
        "student": _str(top["student"], "student", 64),
        "test_date": _str(top["test_date"], "test_date", 32),
        "my_score": _num(top["my_score"], "my_score"),
        "avg_score": _num(top["avg_score"], "avg_score"),
        "rank_signal": _int(top["rank_signal"], "rank_signal", 1, 5),
        "comment": _str(top["comment"], "comment", 3000, multiline=True, required=False),
    }
    units = []
    for i, item in enumerate(_list(top["units"], "units", 1, MAX_FORMATIVE_UNITS)):
        path = "units[%d]" % i
        u = _obj(item, ("name", "my_score", "average"), path)
        units.append(
            {
                "name": _str(u["name"], path + ".name", 80),
                "my_score": _num(u["my_score"], path + ".my_score"),
                "average": _num(u["average"], path + ".average"),
            }
        )
    out["units"] = units
    out["difficulty"] = _report_difficulty(top["difficulty"])
    out["questions"] = _report_questions(top["questions"], "unit")
    return out
