#!/usr/bin/env python3
"""Strict M21 scoring; saved evidence is read only, exports contain no run text."""
import argparse
from collections import Counter
from dataclasses import dataclass, field
import json
from pathlib import Path
import re

CRATE = Path(__file__).resolve().parent.parent
KINDS = ('title', 'parts', 'student_titles', 'unknown', 'next', 'sources', 'closing', 'text')


class Unsupported(ValueError):
    pass


def norm(text):
    """Legacy shown_unchanged normalisation, separate from strict field equality."""
    return ' '.join(text.split())


def configured_policy(request='Q01'):
    return json.loads((Path(__file__).parent / 'checks.json').read_text()) [request]['fidelity_policy']


def validate(policy):
    expected = {
        'contract': 'M21', 'rows': 'one_to_one_whole_fields',
        'bindings': ['part', 'student', 'header'], 'unknown_structure': 'unknown',
        'unmatched_rows': 'reject', 'unmatched_required_lines': 'reject', 'title': 'retain',
        'commentary_state_labels': 'reject_word_table', 'optional_nonrows': ['sources','closing'],
        'normalization': {'surrounding_whitespace': True, 'bars': ['|','｜'],
                          'line_break_encoding': ' / ', 'outer_fence': True},
        'structure': {'tables': 'reference_headers', 'heading_markers': ['#','**'],
                      'list_marker': '- ', 'bullet_separator': ' · ',
                      'student_heading_suffix': ' 학원 내 처리',
                      'student_heading_values': 'first_column', 'text_without_rows': 'whole_lines'},
        'provenance': {'source_prefix': 'READ_SOURCES','closing': 'CLOSING','none': 'NONE'},
    }
    if not isinstance(policy, dict) or set(policy) != set(expected) | {'required_parts','commentary_max_lines','state_labels'}:
        raise Unsupported('unsupported_policy')
    if any(policy[k] != v for k,v in expected.items()):
        raise Unsupported('unsupported_policy_rule')
    if (type(policy['commentary_max_lines']) is not int or policy['commentary_max_lines'] != 1
            or len(policy['required_parts']) != 2 or len(set(policy['required_parts'])) != 2
            or any(not isinstance(p,str) or not p for p in policy['required_parts'])):
        raise Unsupported('unsupported_policy_parameter')
    state = policy['state_labels']
    if (set(state) != {'path','constants'} or state['path'] != 'src/display_labels.rs'
            or not isinstance(state['constants'],list) or not state['constants']):
        raise Unsupported('unsupported_state_policy')
    words = vocabulary(policy)
    if any(name not in words for name in state['constants']):
        raise Unsupported('unavailable_state_word')


def vocabulary(policy):
    source = (CRATE / policy['state_labels']['path']).read_text(encoding='utf-8')
    return {name: json.loads(value) for name, value in re.findall(
        r'pub const (\w+): &str =\s*("(?:\\.|[^"\\])*");', source)}


def value(text, policy):
    # Do not collapse internal spaces, remove punctuation or match substrings.
    encoding = policy['normalization']['line_break_encoding']
    return tuple(part.strip() for part in text.replace('\r\n', '\n').replace('\n', encoding).split(encoding))


def heading(text, policy):
    text = text.strip()
    if '#' in policy['structure']['heading_markers']:
        text = re.sub(r'^#{1,6}\s+', '', text)
    if '**' in policy['structure']['heading_markers'] and text.startswith('**') and text.endswith('**'):
        text = text[2:-2].strip()
    return text


def statement(text, policy):
    marker = policy['structure']['list_marker']
    text = text.strip()
    return value(text[len(marker):] if text.startswith(marker) else text, policy)


def lines_of(text, policy):
    lines = [line.strip() for line in text.replace('\r\n', '\n').split('\n')]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    fences = [i for i,line in enumerate(lines) if line.startswith('```')]
    if fences:
        if (not policy['normalization']['outer_fence'] or len(fences) != 2
                or not re.fullmatch(r'```(?:text|markdown)?',lines[fences[0]])
                or lines[fences[1]] != '```'):
            raise Unsupported('unsupported_fence')
        # Commentary outside a single enclosing fence is still parsed/counts.
        lines = [line for i,line in enumerate(lines) if i not in fences]
    return lines


def fields_of(line, policy):
    bars = policy['normalization']['bars']
    for bar in bars[1:]:
        line = line.replace(bar, bars[0])
    if bars[0] in line:
        return [part.strip() for part in line.strip().strip(bars[0]).split(bars[0])]
    sep = policy['structure']['bullet_separator']
    if line.startswith('(') and line.endswith(')') and sep in line:
        return [part.strip() for part in line[1:-1].split(sep)]
    if line.startswith(policy['structure']['list_marker']) and sep in line:
        return [part.strip() for part in line[2:].split(sep)]
    return None


@dataclass
class Document:
    rows: list = field(default_factory=list)  # (part, student, header, whole values)
    headers: set = field(default_factory=set)
    parts: set = field(default_factory=set)
    students: set = field(default_factory=set)
    facts: list = field(default_factory=list)  # (part, student, whole line)
    other: dict = field(default_factory=lambda: {kind: [] for kind in KINDS})
    commentary: list = field(default_factory=list)
    invalid_rows: int = 0


def parse(text, policy, reference=None):
    words = vocabulary(policy)
    provenance = policy['provenance']
    source_prefix, closing, none = (words[provenance[k]] for k in ('source_prefix','closing','none'))
    lines = lines_of(text, policy)
    if not any(lines):
        raise Unsupported('empty_text')
    doc = Document()
    required = policy['required_parts']
    part = student = ''
    header = None
    footer = False
    known_parts = set() if reference is None else reference.parts
    known_students = set() if reference is None else reference.students
    # Student headings are derived from source row values, not roster fixtures.
    first_values = {f[0] for line in lines if (f := fields_of(line, policy)) and len(f) > 1}
    if reference is not None:
        first_values = {row[3][0][0] for row in reference.rows if row[3]}
    suffix = policy['structure']['student_heading_suffix']
    title = None if reference is None else reference.other['title'][0] if reference.other['title'] else None
    i = 0
    while i < len(lines):
        line = lines[i]
        i += 1
        if not line:
            continue
        clean = heading(line, policy)
        if clean == closing:
            doc.other['closing'].append(statement(clean, policy))
            footer = False
            part = student = ''
            header = None
            continue
        if clean.startswith(source_prefix) or footer:
            optional_block = '' if reference is None else norm(' '.join(' / '.join(v) for v in reference.other['sources']))
            known_footer = reference is None or (' '+norm(clean)+' ') in (' '+optional_block+' ')
            if fields_of(line, policy) is None and known_footer and clean not in known_parts and clean not in required:
                doc.other['sources'].append(statement(clean, policy))
                footer = True
                part = student = ''
                header = None
                continue
            footer = False
        raw_fields = fields_of(line, policy)
        if raw_fields is not None:
            if all(re.fullmatch(r':?-+:?', f) for f in raw_fields):
                continue  # Markdown header separator, not an answer row.
            if header is not None and len(raw_fields) < len(header):
                while i < len(lines) and len(raw_fields) < len(header):
                    following = lines[i]
                    if heading(following, policy) in known_parts | set(required) | known_students:
                        break
                    line += '\n' + following
                    i += 1
                    raw_fields = fields_of(line, policy)
            normalized = tuple(value(f, policy) for f in raw_fields)
            if reference is None:
                if header is None:
                    if len(normalized) < 2 or any(not any(f) for f in normalized):
                        raise Unsupported('invalid_header')
                    header = normalized
                    doc.headers.add(header)
                    continue
                if normalized == header:
                    continue
                if len(normalized) != len(header):
                    raise Unsupported('reference_row_width')
            else:
                if normalized in reference.headers:
                    header = normalized
                    doc.headers.add(header)
                    continue
                if header is None or len(normalized) != len(header):
                    doc.invalid_rows += 1
            # Support a literal break in the last cell only when its continuation
            # is unambiguously a prefix of a designed complete value.
            if reference is not None and header is not None:
                possible = [r[3][-1] for r in reference.rows if r[2] == header and r[3][:-1] == normalized[:-1]]
                while i < len(lines) and any(len(p) > len(normalized[-1]) and p[:len(normalized[-1])] == normalized[-1] for p in possible):
                    following = lines[i]
                    if fields_of(following, policy) is not None or heading(following, policy) in known_parts | set(required) | known_students:
                        break
                    normalized = normalized[:-1] + (normalized[-1] + value(following, policy),)
                    i += 1
            doc.rows.append((part, student, header, normalized))
            continue
        if not doc.other['title'] and (reference is None or statement(clean, policy) == title):
            doc.other['title'].append(statement(clean, policy))
            if reference is not None and ('', '', statement(clean, policy)) in reference.facts:
                doc.facts.append(('', '', statement(clean, policy)))
            continue
        if part in required and clean not in known_parts | set(required):
            kind = 'unknown' if part == required[0] else 'next'
            fact = (part, student, statement(clean, policy))
            doc.facts.append(fact)
            doc.other[kind].append(fact[2])
            continue
        is_student = clean in known_students or clean.endswith(suffix) or any(clean == f or clean.startswith(f+' ') for f in first_values)
        next_line = next((x for x in lines[i:] if x), '')
        is_part = (clean in known_parts or clean in required or line.startswith('#')
                   or line.startswith('**') or (not is_student and fields_of(next_line, policy) is not None))
        if is_student:
            student = clean
            doc.students.add(clean)
            doc.other['student_titles'].append(statement(clean, policy))
        elif is_part or (reference is None and header is None and not doc.rows and clean != none and part not in required):
            part, student, header = clean, '', None
            doc.parts.add(clean)
            doc.other['parts'].append(statement(clean, policy))
        elif part in required:
            kind = 'unknown' if part == required[0] else 'next'
            fact = (part, student, statement(clean, policy))
            doc.facts.append(fact)
            doc.other[kind].append(fact[2])
        elif reference is None:
            if clean == none:
                doc.facts.append((part, student, statement(clean, policy)))
            elif doc.rows:
                raise Unsupported('ambiguous_reference_prose')
            else:
                doc.facts.append((part, student, statement(clean, policy)))
                doc.other['text'].append(statement(clean, policy))
        else:
            candidate = (part, student, statement(clean, policy))
            if candidate in reference.facts:
                doc.facts.append(candidate)
                doc.other['text'].append(candidate[2])
            else:
                doc.commentary.append(statement(clean, policy))
    if reference is None:
        if doc.headers and not doc.rows:
            raise Unsupported('header_without_data')
        if not doc.rows and not doc.facts:
            # Tableless designs compare all non-provenance lines as complete facts.
            doc.facts = [('', '', statement(heading(x, policy), policy)) for x in lines
                         if x and statement(heading(x, policy), policy) not in doc.other['sources'] and heading(x, policy) != closing]
            doc.parts.clear()
            doc.other['parts'].clear()
            doc.other['text'] = [fact[2] for fact in doc.facts]
    return doc


def compare(source, answer, policy):
    reference = parse(source, policy)
    candidate = parse(answer, policy, reference)
    expected, actual = Counter(reference.rows), Counter(candidate.rows)
    missing, extra = expected - actual, actual - expected
    rules = set()
    # Rule 1 is value/multiplicity equality; rule 2 names context-only mismatches.
    projected_expected = Counter(r[3] for r in reference.rows)
    projected_actual = Counter(r[3] for r in candidate.rows)
    if projected_expected - projected_actual:
        rules.add(1)
    if missing and not (projected_expected - projected_actual):
        rules.add(2)
    if reference.other['title'] != candidate.other['title']:
        rules.add(2)
    expected_facts, actual_facts = Counter(reference.facts), Counter(candidate.facts)
    if expected_facts - actual_facts:
        rules.add(3)
    if extra or candidate.invalid_rows or actual_facts - expected_facts:
        rules.add(4)
    words = vocabulary(policy)
    labels = [words[name] for name in policy['state_labels']['constants']]
    if (len(candidate.commentary) > policy['commentary_max_lines']
            or any(label in component for line in candidate.commentary for component in line for label in labels)):
        rules.add(5)
    shared = expected & actual
    nonrows = {kind: {'total': len(reference.other[kind]),
                     'kept': sum((Counter(reference.other[kind]) & Counter(candidate.other[kind])).values())}
               for kind in KINDS}
    result = {'rows_total': len(reference.rows), 'rows_kept': sum(shared.values()),
              'cells_total': sum(len(r[3]) for r in reference.rows),
              'cells_kept': sum(len(r[3]) * count for r,count in shared.items()),
              'nonrows': nonrows, 'added_lines': len(candidate.commentary),
              'added_chars': sum(len(' / '.join(line)) for line in candidate.commentary)}
    return {'facts_kept': 'FAIL' if rules else 'pass', 'failed_rules': sorted(rules), 'fidelity': result}


def evaluate(server, answer, policy):
    if not isinstance(server, str) or not server.strip() or not isinstance(answer, str) or not answer.strip():
        return {'facts_kept': 'unknown', 'failed_rules': []}
    try:
        validate(policy)
        return compare(server, answer, policy)
    except (Unsupported, KeyError, TypeError, OSError):
        return {'facts_kept': 'unknown', 'failed_rules': []}


def score_saved(path):
    import run
    directory = Path(path)
    metadata = json.loads((directory / 'measurement.json').read_text(encoding='utf-8'))
    specs = json.loads((Path(__file__).parent / 'checks.json').read_text(encoding='utf-8'))
    events = [json.loads(line) for line in (directory / 'claude.jsonl').read_text(encoding='utf-8').splitlines()]
    result = run.measure(events)
    return {'shown_unchanged': result['verbatim'], **evaluate(result['display'], result['answer'], specs[metadata['request']]['fidelity_policy'])}


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
