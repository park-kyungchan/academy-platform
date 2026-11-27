"""Line-break tokens, number formatting, text fitting and wrapping (pure Python)."""

import re

# A line-break token: \r\n or \n, or the escaped forms (backslash-r backslash-n, backslash-n),
# with one or two backslashes.
_TOKEN = re.compile(r"(?:\\{1,2}r)?\\{1,2}n|\r\n|\n|\r")

ELLIPSIS = "..."


def split_lines(text):
    """Split at every line-break token; the token itself is dropped, never printed."""
    return [part.rstrip() for part in _TOKEN.split(text)]


def single_line(text):
    """One line from a string: every line-break token becomes one space."""
    return " ".join(part.strip() for part in _TOKEN.split(text) if part.strip())


def format_number(value):
    """Integer-valued numbers print without a fraction, others with one decimal."""
    number = float(value)
    if number == int(number):
        return str(int(number))
    return "%.1f" % number


def fit_line(measure, text, size, width, min_size=6.0):
    """Shrink `size` down to `min_size` until `text` fits `width`; then cut with an ellipsis."""
    current = float(size)
    while measure(text, current) > width and current > min_size:
        current = max(min_size, current - 0.5)
    if measure(text, current) <= width:
        return text, current
    lo, hi = 0, len(text)
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if measure(text[:mid].rstrip() + ELLIPSIS, current) <= width:
            lo = mid
        else:
            hi = mid - 1
    return text[:lo].rstrip() + ELLIPSIS, current


def wrap_text(measure, text, size, width):
    """Greedy wrap of every line of `text` to `width`; an empty line stays an empty line."""
    out = []
    for paragraph in split_lines(text):
        if not paragraph:
            out.append("")
            continue
        current = ""
        for ch in paragraph:
            if measure(current + ch, size) <= width or not current:
                current += ch
                continue
            space = current.rfind(" ")
            if space > 0 and ch != " ":
                out.append(current[:space].rstrip())
                current = current[space + 1 :] + ch
            else:
                out.append(current.rstrip())
                current = ch.lstrip()
        out.append(current.rstrip())
    return out
