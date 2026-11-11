#!/usr/bin/env python3
"""Strict M21 scoring; saved evidence is read only, exports contain no run text."""
import argparse
from collections import Counter
from dataclasses import dataclass
from functools import lru_cache
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
        'structure': {'tables': 'reference_spaced_headers', 'heading_markers': ['#','**'],
                      'list_marker': '- ', 'bullet_separator': ' · ',
                      'student_heading_suffix': ' 학원 내 처리',
                      'student_heading_level': 2, 'answer_context': 'reference_markers_only', 'text_without_rows': 'whole_lines',
                      'plain_part_before_header': 'shared_first_header_or_new_width',
                      'unmarked_cell_continuation': 'last_cell'},
        'provenance': {'source_prefix': 'READ_SOURCES','closing': 'CLOSING'},
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
    return word_table(policy['state_labels']['path'])


@lru_cache(maxsize=8)
def word_table(path):
    source = (CRATE / path).read_text(encoding='utf-8')
    return {name: json.loads(value) for name, value in re.findall(
        r'pub const (\w+): &str =\s*("(?:\\.|[^"\\])*");', source)}


def value(text, policy):
    return ' / '.join(part.strip() for part in text.replace('\r\n','\n').replace('\n',' / ').split(' / '))


def heading(text, policy):
    text = re.sub(r'^#{1,6}\s+', '', text.strip())
    return text[2:-2].strip() if text.startswith('**') and text.endswith('**') else text


def statement(text, policy):
    text = text.strip()
    return text[2:].strip() if text.startswith(policy['structure']['list_marker']) else text


def physical_lines(text, policy, reference=False):
    lines = [line.strip() for line in text.replace('\r\n','\n').split('\n') if line.strip()]
    fences = [i for i,line in enumerate(lines) if line.startswith('```')]
    if fences:
        valid = (len(fences)==2 and re.fullmatch(r'```(?:text|markdown)?',lines[fences[0]])
                 and lines[fences[1]]=='```')
        if reference and not valid:
            raise Unsupported('unsupported_reference_fence')
        if valid:
            lines = [line for i,line in enumerate(lines) if i not in fences]
    return lines


def fields_of(line, policy):
    # Spaced delimiters keep unspaced literal full-width bars inside cells.
    body = line.strip().strip('|｜').strip()
    fields = re.split(r'[ \t]+[|｜][ \t]+', body)
    if len(fields)>1:
        return fields
    sep = policy['structure']['bullet_separator']
    if line.startswith('(') and line.endswith(')') and sep in line:
        return line[1:-1].split(sep)
    if line.startswith('- ') and sep in line:
        return line[2:].split(sep)
    return None


@dataclass(frozen=True)
class Unit:
    kind: str
    value: object
    part: str = ''
    student: str = ''
    header: tuple = ()


def parse_reference(text, policy):
    lines = physical_lines(text,policy,True)
    if not lines:
        raise Unsupported('empty_reference')
    words = vocabulary(policy)
    required = policy['required_parts']
    prefix,closing = (words[policy['provenance'][key]] for key in ('source_prefix','closing'))
    units = [Unit('title',heading(lines[0],policy))]
    part=student=''
    header=()
    i=1
    def boundary(index):
        line=lines[index]; clean=heading(line,policy)
        following=fields_of(lines[index+1],policy) if index+1<len(lines) else None
        return (clean in required or clean.startswith(prefix) or clean==closing
                or line.startswith(('#','**')) or clean.endswith(policy['structure']['student_heading_suffix'])
                or (following is not None and (not header or value(following[0],policy)==header[0] or len(following)!=len(header))))
    while i<len(lines):
        line=lines[i]; clean=heading(line,policy); f=fields_of(line,policy)
        if clean.startswith(prefix):
            group=[]
            while i<len(lines) and heading(lines[i],policy)!=closing:
                if fields_of(lines[i],policy) is not None:
                    raise Unsupported('row_in_reference_footer')
                group.append(lines[i]); i+=1
            units.append(Unit('sources',norm(' '.join(group)))); continue
        if clean==closing:
            units.append(Unit('closing',clean)); i+=1; continue
        if f is None and boundary(i):
            is_student=(clean.endswith(policy['structure']['student_heading_suffix']) or line.startswith('## '))
            if is_student:
                student=clean; units.append(Unit('student_titles',clean,part));
            else:
                part=clean; student=''; header=(); units.append(Unit('parts',clean))
            i+=1; continue
        if f is None and header and i+1<len(lines) and fields_of(lines[i+1],policy) is not None:
            line += '\n'+lines[i+1]; f=fields_of(line,policy); i+=1
        if f is not None:
            if all(re.fullmatch(r':?-+:?',v.strip()) for v in f):
                if not header: raise Unsupported('orphan_header_separator')
                i+=1; continue
            if not header:
                header=tuple(value(v,policy) for v in f)
                if len(header)<2: raise Unsupported('invalid_reference_header')
                units.append(Unit('header',header,part,student)); i+=1; continue
            while (len(f)<len(header) or (line.startswith(('|','｜')) and not line.endswith(('|','｜')))) and i+1<len(lines) and (not boundary(i+1) or (line.startswith(('|','｜')) and not line.endswith(('|','｜')) and lines[i+1].endswith(('|','｜')))):
                i+=1; line+='\n'+lines[i]; f=fields_of(line,policy)
            if len(f)!=len(header): raise Unsupported('reference_row_width')
            i+=1
            while not line.startswith(('|','｜')) and i<len(lines) and fields_of(lines[i],policy) is None and not boundary(i):
                # An unmarked continuation belongs to the last cell. This same
                # complete value is reconstructed by logical_lines on both sides.
                f[-1]+='\n'+lines[i]; i+=1
            vals=tuple(value(v,policy) for v in f)
            units.append(Unit('header' if vals==header else 'row',vals,part,student,() if vals==header else header))
            continue
        kind = 'unknown' if part==required[0] else 'next' if part==required[1] else 'text'
        units.append(Unit(kind,statement(line,policy),part,student)); i+=1
    return tuple(units)


@lru_cache(maxsize=8192)
def pattern(unit):
    if unit.kind in ('header','row'):
        cells=[r'(?:[ \t]+/[ \t]+|\n)'.join(re.escape(v) for v in cell.split(' / ')) for cell in unit.value]
        pipe=r'[|｜]?[ \t]*'+r'[ \t]*[|｜][ \t]*'.join(cells)+r'[ \t]*[|｜]?'
        bullet=(r'\('+r' · '.join(cells)+r'\)') if unit.kind=='header' else (r'- '+r' · '.join(cells))
        body='(?:'+pipe+'|'+bullet+')'
    elif unit.kind in ('title','parts','student_titles'):
        body=r'(?:#{1,6}[ \t]+)?(?:\*\*)?'+re.escape(unit.value)+r'(?:\*\*)?'
    elif unit.kind=='sources':
        body=r'\s+'.join(re.escape(v) for v in unit.value.split())
    else:
        body=r'(?:- )?'+re.escape(unit.value)
    return re.compile(r'\A'+body+r'(?=\n|\Z)')


@lru_cache(maxsize=1024)
def normalizers(units):
    indexed={}
    for u in units:
        key=(u.kind,u.value)
        initial=u.value[0][:1] if u.kind in ('row','header') else u.value[:1]
        starts={initial}
        starts.update(('|','｜','-','(') if u.kind in ('row','header') else ('#','*') if u.kind in ('title','parts','student_titles') else ('-',))
        for char in starts:
            if char:
                indexed.setdefault(char,{})[key]=pattern(u)
    return {char:tuple(choices.items()) for char,choices in indexed.items()}


def logical_lines(text, units, policy):
    """Only reference unit patterns rejoin cells; identical code for both sides."""
    remaining='\n'.join(physical_lines(text,policy))
    patterns=normalizers(units)
    result=[]
    while remaining:
        matched=False
        for key,regex in patterns.get(remaining[0],()):
            hit=regex.match(remaining)
            if hit:
                result.append((key,remaining[:hit.end()])); remaining=remaining[hit.end():].lstrip('\n'); matched=True; break
        if not matched:
            line,sep,remaining=remaining.partition('\n')
            # Table separator syntax belongs to the table-header presentation.
            fs=fields_of(line,policy)
            if fs and all(re.fullmatch(r':?-+:?',f.strip()) for f in fs): continue
            result.append((None,line))
    return result


@lru_cache(maxsize=1024)
def reference_units(source, serialized):
    policy=json.loads(serialized); validate(policy)
    units=parse_reference(source,policy)
    # Every nonempty reference line must resolve to its assigned unit, including
    # continuations; there is no tableless fallback that discards statements.
    tokens=logical_lines(source,units,policy)
    if Counter(key for key,raw in tokens)!=Counter((u.kind,u.value) for u in units):
        raise Unsupported('unassigned_reference_line')
    return units


def compare(units, answer, policy):
    available=[True]*len(units); added=[]; seen_rows=Counter(); by_key={}
    for index,unit in enumerate(units): by_key.setdefault((unit.kind,unit.value),[]).append(index)
    part=student=''; header=(); rules=set()
    for key,raw in logical_lines(answer,units,policy):
        found=None
        if key:
            kind,val=key
            if kind=='row': seen_rows[val]+=1
            for index in by_key.get(key,()):
                if not available[index]: continue
                unit=units[index]
                valid=(kind in ('title','parts','sources','closing')
                       or (kind=='student_titles' and unit.part==part)
                       or (kind=='header' and (unit.part,unit.student)==(part,student))
                       or (kind=='row' and (unit.part,unit.student,unit.header)==(part,student,header))
                       or (kind in ('text','unknown','next') and (unit.part,unit.student)==(part,student)))
                if valid: found=index; break
        if found is None:
            added.append((key,raw)); continue  # Never changes context.
        available[found]=False; unit=units[found]
        if unit.kind=='parts': part=unit.value; student=''; header=()
        elif unit.kind=='student_titles': student=unit.value
        elif unit.kind=='header': header=unit.value
        elif unit.kind in ('sources','closing'): part=student=''; header=()
    expected_rows=Counter(u.value for u in units if u.kind=='row')
    for unit,missing in zip(units,available):
        if not missing or unit.kind in policy['optional_nonrows']: continue
        if unit.kind=='row': rules.add(1 if seen_rows[unit.value]<expected_rows[unit.value] else 2)
        elif unit.kind in ('unknown','next','text'): rules.add(3)
        else: rules.add(2)
    words=vocabulary(policy); labels=[words[name] for name in policy['state_labels']['constants']]
    if any((key and key[0] in ('row','header')) or any(bar in raw for bar in ('|','｜')) or ' · ' in raw for key,raw in added): rules.add(4)
    if len(added)>policy['commentary_max_lines'] or any(label in raw for key,raw in added for label in labels): rules.add(5)
    nonrows={kind:{'total':sum(u.kind==kind for u in units),'kept':sum(u.kind==kind and not missing for u,missing in zip(units,available))} for kind in KINDS}
    return {'facts_kept':'FAIL' if rules else 'pass','failed_rules':sorted(rules),'fidelity':{
        'rows_total':sum(u.kind=='row' for u in units),'rows_kept':sum(u.kind=='row' and not missing for u,missing in zip(units,available)),
        'cells_total':sum(len(u.value) for u in units if u.kind=='row'),
        'cells_kept':sum(len(u.value) for u,missing in zip(units,available) if u.kind=='row' and not missing),
        'nonrows':nonrows,'added_lines':len(added),'added_chars':sum(len(raw) for key,raw in added)}}


def evaluate(server, answer, policy):
    if not isinstance(server,str) or not isinstance(answer,str):
        return {'facts_kept':'unknown','failed_rules':[]}
    try:
        serialized=json.dumps(policy,sort_keys=True)
        units=reference_units(server,serialized)
    except (Unsupported,KeyError,TypeError,OSError):
        return {'facts_kept':'unknown','failed_rules':[]}
    # Answer content is never a reason to return unknown.
    return compare(units,answer,policy)


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
