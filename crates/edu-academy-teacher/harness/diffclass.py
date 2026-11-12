#!/usr/bin/env python3
"""Counts-only descriptions of saved answer differences; never changes M21."""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import unicodedata

import fidelity
import run

MISSING = ('equal_after_marker_strip', 'equal_after_spacing_punctuation',
           'merged_or_split', 'reworded_high', 'reworded_low', 'absent')
ADDED = ('restyled_reference_unit', 'question_to_user', 'offer_of_help',
         'commentary_with_state_label', 'commentary_other')
MAX_TEXT_BYTES = 65536
MAX_LINES = 512
MAX_UNITS = 512
MAX_TOKENS = 8192
MAX_WORK = 2_000_000
MAX_METADATA_BYTES = 4 * 1024 * 1024
MAX_STREAM_BYTES = 8 * 1024 * 1024
MAX_LOG_BYTES = 1024 * 1024
MAX_ROWS = 256
MAX_CANDIDATES = 64
MAX_EVENTS = 4096
NUMBER = re.compile(r'\d+(?:[-.:]\d+)*')


class Unknown(ValueError):
    pass


class Budget:
    def __init__(self):
        self.remaining = MAX_WORK

    def spend(self, amount):
        self.remaining -= amount
        if self.remaining < 0:
            raise Unknown('comparison_budget')


def tokens(text):
    return re.findall(r'\w+', unicodedata.normalize('NFC', marker_strip(text)).casefold())


def marker_strip(text):
    lines = []
    for line in text.strip().splitlines():
        old = None
        while line != old:
            old = line
            line = re.sub(r'^\s*(?:[-*+•]\s+|(?:\d+|[A-Za-z])[.)]\s+|#{1,6}\s+)', '', line)
        for pattern in (r'\*\*(.+?)\*\*', r'__(.+?)__', r'\*(.+?)\*', r'_(.+?)_', r'`([^`]+)`'):
            line = re.sub(pattern, r'\1', line)
        lines.append(re.sub(r'[:：]\s*$', '', line).strip())
    return '\n'.join(lines)


def punctuation_strip(text):
    return ''.join(c for c in marker_strip(text)
                   if not c.isspace() and not unicodedata.category(c).startswith('P'))


def unit_text(unit):
    return ' | '.join(unit.value) if isinstance(unit.value, tuple) else unit.value


def overlap(left, right, budget):
    budget.spend(len(left) + len(right) + 1)
    if not left or not right:
        return 0.0
    common = sum((Counter(left) & Counter(right)).values())
    return 2 * common / (len(left) + len(right))


def subsequence(needle, haystack, budget):
    budget.spend(len(needle) + len(haystack) + 1)
    if not needle:
        return False
    pos = 0
    for token in haystack:
        if token == needle[pos]:
            pos += 1
            if pos == len(needle):
                return True
    return False


def missing_class(text, lines, other_units, budget):
    """Marker equality precedes punctuation equality and overlap pairing."""
    for strip, category in ((marker_strip, MISSING[0]), (punctuation_strip, MISSING[1])):
        for line in lines:
            budget.spend(len(text) + len(line) + 1)
            if strip(text) == strip(line):
                return category
    needle = tokens(text)
    line_tokens = [tokens(line) for line in lines]
    scores = [overlap(needle, values, budget) for values in line_tokens]
    # All tokens in order across any two lines, with both contributing.
    for i, first in enumerate(line_tokens):
        if subsequence(needle, first, budget):
            for other in other_units:
                values = tokens(other)
                if (subsequence(needle + values, first, budget)
                        or subsequence(values + needle, first, budget)):
                    return MISSING[2]
        for second in line_tokens[i + 1:]:
            if (not subsequence(needle, first, budget)
                    and not subsequence(needle, second, budget)
                    and subsequence(needle, first + second, budget)):
                return MISSING[2]
    best = max(scores, default=0.0)
    return 'reworded_high' if best >= 0.7 else 'reworded_low' if best >= 0.3 else 'absent'


def added_class(text, references, state_labels):
    if any(marker_strip(text) == marker_strip(reference)
           or punctuation_strip(text) == punctuation_strip(reference) for reference in references):
        return 'restyled_reference_unit'
    if (re.search(r'\b(?:can i help|let me know|i can help)\b', text, re.I)
            or re.search(r'원하시면|필요하시면|도와드|알려드릴', text)):
        return 'offer_of_help'
    if re.search(r'[?？]\s*$|(?:까요|나요|습니까|인지요)[.?？]?\s*$', text):
        return 'question_to_user'
    if any(label in text for label in state_labels):
        return 'commentary_with_state_label'
    return 'commentary_other'


def choice_labels(source, kind):
    if kind != 'ask':
        return []
    options = [line[2:] for line in source.splitlines() if line.startswith('- ')]
    if not options or any(option.count(': ') > 1 for option in options):
        raise Unknown('ask_labels_unavailable')
    return [option.split(': ', 1)[0] for option in options]


def inside_fence(answer):
    lines = answer.strip().splitlines()
    return (len(lines) >= 2 and bool(re.fullmatch(r'```[A-Za-z]*\s*', lines[0]))
            and lines[-1].strip() == '```'
            and sum(line.lstrip().startswith('```') for line in lines) == 2)


def analyze(source, answer, policy, kind='result'):
    """Export a closed schema: no matched strings, unit values or raw metadata."""
    result = {'analysis': 'unknown', 'reason_code': None, 'facts_kept': 'unknown',
              'display_kind': kind if kind in ('result', 'ask', 'stop') else 'unknown',
              'reference_units': None, 'missing_units': None, 'added_lines': None,
              'missing_classes': None, 'added_classes': None,
              'choices_present': None, 'states_same_count': None, 'numbers_same_count': None,
              'answer_bytes': None, 'reference_bytes': None, 'answer_lines': None,
              'reference_lines': None, 'inside_one_code_fence': None}
    try:
        if not isinstance(source, str) or not isinstance(answer, str):
            raise Unknown('missing_evidence')
        result.update(answer_bytes=len(answer.encode('utf-8')), reference_bytes=len(source.encode('utf-8')),
                      answer_lines=len(answer.splitlines()), reference_lines=len(source.splitlines()))
        if max(result['answer_bytes'], result['reference_bytes']) > MAX_TEXT_BYTES:
            raise Unknown('text_bytes_limit')
        if max(result['answer_lines'], result['reference_lines']) > MAX_LINES:
            raise Unknown('line_limit')
        if max(len(tokens(source)), len(tokens(answer))) > MAX_TOKENS:
            raise Unknown('token_limit')
        measured = fidelity.evaluate(source, answer, policy)
        result['facts_kept'] = measured['facts_kept']
        if measured['facts_kept'] == 'unknown':
            raise Unknown(measured.get('reason_code') or 'strict_measure_unknown')
        units = fidelity.reference_units(source, json.dumps(policy, sort_keys=True))
        if len(units) > MAX_UNITS:
            raise Unknown('reference_units_limit')
        labels = sorted({fidelity.vocabulary(policy)[name] for name in policy['state_labels']['constants']})
        choices = choice_labels(source, kind)
        result.update(reference_units=len(units), choices_present=all(label in answer for label in choices),
                      states_same_count=all(source.count(label) == answer.count(label) for label in labels),
                      numbers_same_count=Counter(NUMBER.findall(source)) == Counter(NUMBER.findall(answer)),
                      inside_one_code_fence=inside_fence(answer))
        missing, added = Counter(), []
        if measured['facts_kept'] != 'pass':
            path = fidelity.diagnostic_path(units, answer, policy)
            kept = Counter(unit for unit, _, _ in path if unit is not None)
            missing = Counter(units) - kept
            added = [raw for unit, raw, _ in path if unit is None]
        budget = Budget()
        lines = fidelity.physical_lines(answer, policy)
        references = [unit_text(unit) for unit in units]
        missing_counts, added_counts = dict.fromkeys(MISSING, 0), dict.fromkeys(ADDED, 0)
        for unit, count in missing.items():
            others = [unit_text(other) for other in units if other is not unit]
            category = missing_class(unit_text(unit), lines, others, budget)
            missing_counts[category] += count
        for raw in added:
            budget.spend(sum(len(s) for s in references) + len(raw) + 1)
            added_counts[added_class(raw, references, labels)] += 1
        result.update(analysis='complete', missing_units=sum(missing.values()), added_lines=len(added),
                      missing_classes=missing_counts, added_classes=added_counts)
    except Unknown as exc:
        result.update(analysis='unknown', reason_code=exc.args[0], missing_classes=None, added_classes=None)
    except (ValueError, TypeError, KeyError, OSError, RecursionError):
        result.update(analysis='unknown', reason_code='unsupported_evidence', missing_classes=None, added_classes=None)
    return result


def read_bounded(path, limit):
    with Path(path).open('rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise Unknown('saved_file_bytes_limit')
    return data.decode('utf-8')


def identity(row):
    if not isinstance(row, dict):
        raise Unknown('unsupported_row_identity')
    request, tier, mode = (row.get(key) for key in ('request', 'model_tier', 'mode'))
    conversation = row.get('conversation', 1)
    if (request not in ('Q11', 'Q12', 'Q13', 'Q14', 'Q15', 'Q16')
            or tier not in ('low', 'reference') or mode not in ('isolated', 'installed')
            or type(conversation) is not int or not 1 <= conversation <= 1000
            or type(row.get('trial', False)) is not bool
            or type(row.get('turn')) is not int or not 1 <= row['turn'] <= (2 if request in ('Q12', 'Q14') else 1)):
        raise Unknown('unsupported_row_identity')
    return {key: row.get(key, default) for key, default in
            (('request', request), ('turn', 1), ('model_tier', tier), ('mode', mode),
             ('conversation', 1), ('trial', False))}


def saved_fingerprint(result):
    usage = result.get('usage') or {}
    return (result.get('call_count'), result.get('args_bytes'), result.get('unit_counts'),
            result.get('facts_kept'), result.get('elapsed_s'),
            {key: usage.get(key, 'unknown') for key in ('input_tokens', 'output_tokens', 'cache_read_input_tokens', 'cache_creation_input_tokens')})


def row_fingerprint(row):
    return (row.get('calls'), row.get('argument_bytes'), row.get('unit_counts'),
            row.get('facts_kept'), row.get('elapsed_s'), row.get('tokens'))


def resolve(row):
    info = identity(row)
    model = {'low': 'haiku', 'reference': 'sonnet'}[info['model_tier']]
    matches, seen = [], 0
    for root in Path(tempfile.gettempdir()).glob(f"p8-{info['mode']}-{info['request']}-{model}-*"):
        seen += 1
        if seen > MAX_CANDIDATES:
            raise Unknown('candidate_limit')
        directory = root / 'run'
        try:
            saved = json.loads(read_bounded(directory / 'measurement.json', MAX_METADATA_BYTES))
            results = saved.get('results', [])
            if (saved.get('conversation', 1) == info['conversation']
                    and saved.get('trial', info['trial']) == info['trial']
                    and len(results) >= info['turn']
                    and saved_fingerprint(results[info['turn'] - 1]) == row_fingerprint(row)):
                matches.append(directory)
        except Unknown:
            raise
        except (OSError, ValueError, UnicodeError, TypeError, AttributeError, RecursionError):
            continue
    if len(matches) != 1:
        raise Unknown('saved_run_not_unique' if matches else 'saved_run_not_found')
    return matches[0]


def score_directory(directory, expected=None):
    saved = json.loads(read_bounded(directory / 'measurement.json', MAX_METADATA_BYTES))
    if expected is None:
        model = saved.get('model')
        info = identity({'request': saved.get('request'), 'model_tier': {'haiku': 'low', 'sonnet': 'reference'}.get(model),
                         'mode': saved.get('mode'), 'trial': saved.get('trial', False), 'conversation': saved.get('conversation', 1), 'turn': 1})
    else:
        info = identity(expected)
    specs = json.loads((run.HERE / 'checks.json').read_text())
    raw = read_bounded(directory / 'claude.jsonl', MAX_STREAM_BYTES)
    log_lines = read_bounded(directory / 'run-log.jsonl', MAX_LOG_BYTES).splitlines()
    if max(len(raw.splitlines()), len(log_lines)) > MAX_EVENTS:
        raise Unknown('stream_event_limit')
    log = [json.loads(line) for line in log_lines]
    chunks, current = [], []
    for line in raw.splitlines():
        event = json.loads(line)
        if not isinstance(event, dict):
            raise Unknown('malformed_stream')
        current.append(event)
        if event.get('type') == 'result':
            chunks.append(current); current = []
    if current:
        chunks.append(current)
    if len(chunks) > 2:
        raise Unknown('turn_limit')
    requested = [info['turn']] if expected else range(1, len(specs[info['request']]['turns']) + 1)
    results, offset = [], 0
    all_events = [event for chunk in chunks for event in chunk]
    first = run.isolation(all_events, [], mode=info['mode'])['first_request_tokens']
    for turn in range(1, max(requested, default=0) + 1):
        events = chunks[turn - 1] if turn <= len(chunks) else []
        measured = run.measure(events)
        count = sum(str(call['name']).startswith('mcp__academy__') for call in measured['calls'])
        logs = log[offset:offset + count]; offset += count
        if turn not in requested:
            continue
        kind = ('ask' if str(logs[-1].get('code', '')).startswith('ask:') else
                'stop' if logs[-1].get('code') else 'result') if logs else 'unknown'
        policy = specs[info['request']]['turns'][turn - 1]['fidelity_policy']
        analyzed = analyze(measured['display'], measured['answer'], policy, kind)
        results.append({**info, 'turn': turn, 'first_request_tokens': first, **analyzed})
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('paths', nargs='*', type=Path, help='Saved run directories; never printed')
    parser.add_argument('--rows', type=Path, help='Resolve each harness row to its saved evidence')
    parser.add_argument('--output', type=Path, help='Counts-only jsonl, mode 0600; stdout always has identical rows')
    args = parser.parse_args()
    outputs = []
    try:
        if args.rows:
            rows = [json.loads(line) for line in read_bounded(args.rows, MAX_METADATA_BYTES).splitlines()]
            if len(rows) > MAX_ROWS:
                raise Unknown('row_limit')
            for row in rows:
                info = identity(row)
                try:
                    outputs.extend(score_directory(resolve(row), row))
                except (Unknown, OSError, ValueError, KeyError, TypeError, AttributeError, IndexError, UnicodeError, RecursionError) as exc:
                    reason = exc.args[0] if isinstance(exc, Unknown) else 'evidence_unavailable'
                    outputs.append({**info, 'analysis': 'unknown', 'reason_code': reason})
        else:
            if len(args.paths) > MAX_ROWS:
                raise Unknown('row_limit')
            for directory in args.paths:
                outputs.extend(score_directory(directory))
    except (Unknown, OSError, ValueError, KeyError, TypeError, AttributeError, IndexError, UnicodeError, RecursionError) as exc:
        reason = exc.args[0] if isinstance(exc, Unknown) else 'evidence_unavailable'
        outputs = [{'analysis': 'unknown', 'reason_code': reason}]
    encoded = ''.join(json.dumps(row, ensure_ascii=True, sort_keys=True) + '\n' for row in outputs)
    if args.output:
        try:
            fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, 'w', encoding='utf-8') as out:
                out.write(encoded)
        except OSError:
            print(json.dumps({'analysis': 'unknown', 'reason_code': 'output_unavailable'}))
            return 2
    print(encoded, end='')
    return 0 if all(row['analysis'] == 'complete' for row in outputs) else 2


if __name__ == '__main__':
    sys.exit(main())
