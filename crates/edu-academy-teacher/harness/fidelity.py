#!/usr/bin/env python3
"""Offline literal fidelity counts. Never print answer or reference text."""
import argparse
import json
from pathlib import Path
import re

CRATE = Path(__file__).resolve().parent.parent

KINDS = ('title', 'parts', 'student_titles', 'unknown', 'next', 'sources', 'closing')


def norm(text):
    return ' '.join(text.split())


def plain(text):
    """Remove only the Markdown characters named in the packet, then normalise."""
    return norm(re.sub(r'[|\-*#`]', '', text))


def vocabulary():
    source = (CRATE / 'src/display_labels.rs').read_text(encoding='utf-8')
    return {name: json.loads(value) for name, value in re.findall(
        r'pub const (\w+): &str =\s*("(?:\\.|[^"\\])*");', source)}


def parse_reference(text):
    words = vocabulary()
    parts = ('알림', '학생별 할 일', '미확인 사항', '다음 확인')
    headers = [words['ACTION_HEADER'].split(' | '), words['REMINDER_HEADER'].split(' | ')]
    rows, other = [], {kind: [] for kind in KINDS}
    section, footer = None, False
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith('```') or re.fullmatch(r'[| :\-]+', line):
            continue
        if ' | ' in line:
            fields = line.strip('|').strip().split(' | ')
            if fields in headers:
                continue
            if len(fields) != 6:
                raise ValueError('reference_row_field_count')
            rows.append([norm(field) for field in fields])
            continue
        clean = plain(line)
        if clean in parts:
            other['parts'].append(clean)
            section, footer = clean, False
        elif clean == plain(words['CLOSING']):
            other['closing'].append(clean)
        elif clean.startswith(words['READ_SOURCES']) or footer:
            other['sources'].append(clean)
            footer = True
        elif clean.endswith(' 학원 내 처리'):
            other['student_titles'].append(clean)
        elif not other['title']:
            other['title'].append(clean)
        elif section == parts[2]:
            other['unknown'].append(clean)
        elif section == parts[3]:
            other['next'].append(clean)
        elif clean != words['NONE']:
            raise ValueError('unclassified_reference_line')
    return rows, other


def ordered_fields(fields, answer, start=0):
    """Maximum kept fields in order; for each count retain the earliest end.

    Skipping a missing field does not hide later fields. Matches do not overlap
    within one row. Separate rows may reuse an occurrence (a stated limitation).
    """
    ends = {0: start}
    for field in fields:
        updated = dict(ends)
        for count, end in ends.items():
            pos = answer.find(field, end)
            if pos >= 0:
                new_end = pos + len(field)
                updated[count + 1] = min(updated.get(count + 1, len(answer) + 1), new_end)
        ends = updated
    count = max(ends)
    return count, ends[count]


def state_catalog(text):
    words = vocabulary()
    source = (CRATE / 'src/display_labels.rs').read_text(encoding='utf-8')
    verdict = source.split('pub fn verdict(', 1)[1].split('pub fn search_dates', 1)[0]
    names = set(re.findall(r'=> ([A-Z_]+)', verdict)) | {
        'TEACHER', 'SERVER', 'PHYSICAL', 'WAITING', 'UNJUDGED', 'DEFERRED',
        'MISSING_VIDEO', 'MISSING_TITLE', 'NOT_OBSERVED', 'NOT_IN_RANGE',
        'NOT_IN_EITHER', 'NOT_YET', 'NOT_APPLICABLE', 'UNAVAILABLE'}
    catalog = {name: re.escape(words[name]) for name in names if name in words}
    # Qualified teacher states count as a full label, not also its base prefix.
    teacher = words['TEACHER']
    catalog['TEACHER'] += r'(?!\()'
    for match in re.finditer(re.escape(teacher) + r'\(([^)]+)\)', text):
        numbers = '_'.join(re.findall(r'\d+', match.group(1)))
        catalog['TEACHER_DATED_' + numbers] = re.escape(match.group())
    return catalog


def score(server, answer, catalog=None):
    rows, other = parse_reference(server)
    final, stripped = norm(answer), plain(answer)
    counts = [ordered_fields(fields, final)[0] for fields in rows]
    kept = [fields for fields, n in zip(rows, counts) if n == len(fields)]
    cursor, ordered = 0, True
    for fields in kept:
        n, cursor = ordered_fields(fields, final, cursor)
        if n != len(fields):
            ordered = False
            break
    nonrows = {kind: {'total': len(lines), 'kept': sum(line in stripped for line in lines)}
               for kind, lines in other.items()}
    anchors = [plain(field) for row in rows for field in row]
    anchors += [line for lines in other.values() for line in lines]
    if vocabulary()['NONE'] in server.splitlines():
        anchors.append(vocabulary()['NONE'])
    additions = [line for line in (plain(line) for line in answer.splitlines())
                 if line and not any(anchor and anchor in line for anchor in anchors)]
    patterns = state_catalog(norm(server) + " " + final) if catalog is None else catalog
    states = {}
    for name, pattern in sorted(patterns.items()):
        before, after = len(re.findall(pattern, norm(server))), len(re.findall(pattern, final))
        if before or after:
            states[name] = {'server': before, 'answer': after}
    return {'cells_total': sum(map(len, rows)), 'cells_kept': sum(counts),
            'rows_total': len(rows), 'rows_kept': len(kept),
            'rows_order_kept': 'yes' if ordered else 'no', 'nonrows': nonrows,
            'added_lines': len(additions), 'added_chars': sum(map(len, additions)),
            'state_words_changed': states}


def evaluate(server, answer, kind):
    """M19 result; unknown evidence never satisfies a slice acceptance check."""
    if not isinstance(server, str) or not norm(server) or not isinstance(answer, str) or not norm(answer):
        return {'facts_kept': 'unknown'}
    if kind == 'text':
        # Asks/stops have no checklist rows: require their complete delivered text.
        anchors = [plain(line) for line in server.splitlines() if plain(line)]
        additions = [line for line in (plain(line) for line in answer.splitlines())
                     if line and not any(anchor in line for anchor in anchors)]
        return {'facts_kept': 'pass' if norm(server) in norm(answer) and len(additions) <= 1 else 'FAIL'}
    if kind != 'checklist':
        return {'facts_kept': 'unknown'}
    try:
        result = score(server, answer)
    except (ValueError, KeyError):
        return {'facts_kept': 'unknown'}
    if result['nonrows']['parts']['total'] != 4 or result['nonrows']['title']['total'] != 1:
        return {'facts_kept': 'unknown'}
    kept = (result['rows_kept'] == result['rows_total']
            and all(state['server'] == state['answer'] for state in result['state_words_changed'].values())
            and result['nonrows']['unknown']['kept'] == result['nonrows']['unknown']['total']
            and result['added_lines'] <= 1)
    return {'facts_kept': 'pass' if kept else 'FAIL', 'fidelity': result}


def score_saved(path):
    """Only the script reads saved streams; only counts leave this function."""
    import run
    result = run.measure([json.loads(line) for line in
                          (Path(path) / 'claude.jsonl').read_text(encoding='utf-8').splitlines()])
    if not isinstance(result['display'], str) or not isinstance(result['answer'], str):
        raise ValueError('missing_delivered_text_or_final_answer')
    return score(result['display'], result['answer'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('paths', nargs='+', help='Saved run directories (read only)')
    args = parser.parse_args()
    for path in args.paths:
        try:
            print(json.dumps(score_saved(path), ensure_ascii=True, sort_keys=True))
        except (OSError, ValueError, KeyError, TypeError):
            print(json.dumps({'status': 'evidence_unavailable'}))
            return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
