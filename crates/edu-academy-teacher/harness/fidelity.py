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
        'commentary_state_labels': 'reject_word_table', 'commentary_allowed': 'outside_required_parts', 'optional_nonrows': ['sources','closing'],
        'normalization': {'surrounding_whitespace': True, 'bars': ['|','｜'],
                          'line_break_encoding': ' / ', 'outer_fence': True, 'table_separator': 'ignore_whole_lines'},
        'structure': {'tables': 'reference_spaced_headers', 'heading_markers': ['#','**'],
                      'list_marker': '- ', 'bullet_separator': ' · ',
                      'student_heading_suffix': ' 학원 내 처리',
                      'student_heading_level': 2, 'answer_context': 'reference_markers_only', 'text_without_rows': 'whole_lines',
                      'plain_part_before_header': 'shared_first_header_or_new_width',
                      'unmarked_cell_continuation': 'last_cell', 'plain_empty_part': 'heading_followed_by_none'},
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
    if policy['normalization']['table_separator']=='ignore_whole_lines':
        lines=[line for line in lines if not table_separator(line,policy)]
    return lines


def table_separator(line, policy):
    fields=fields_of(line,policy)
    return bool(fields) and all(re.fullmatch(r':?-+:?',cell.strip()) for cell in fields)


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
                or (index+1<len(lines) and lines[index+1]==words['NONE'])
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
                # complete value is matched by assignment search on both sides.
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


def match_options(text, offset, units):
    tail=text[offset:]
    choices=[]
    for key,regex in normalizers(units).get(tail[:1],()):
        hit=regex.match(tail)
        if hit:
            end=offset+hit.end()
            choices.append((key,end))
    # Ordering only accelerates the search; every grouping remains available.
    return sorted(choices,key=lambda item:(-item[1],repr(item[0])))


def compatible(unit, context):
    part,student,header=context
    return (unit.kind in ('title','parts','sources','closing')
            or (unit.kind=='student_titles' and unit.part==part)
            or (unit.kind=='header' and (unit.part,unit.student)==(part,student))
            or (unit.kind=='row' and (unit.part,unit.student,unit.header)==context)
            or (unit.kind in ('text','unknown','next') and (unit.part,unit.student)==(part,student)))


def next_context(unit, context):
    part,student,header=context
    if unit.kind=='parts': return (unit.value,'',())
    if unit.kind=='student_titles': return (part,unit.value,header)
    if unit.kind=='header': return (part,student,unit.value)
    if unit.kind in ('sources','closing'): return ('','',())
    return context


def row_like(key, raw):
    return bool(key and key[0] in ('row','header')) or any(bar in raw for bar in ('|','｜')) or ' · ' in raw


def find_assignment(units, answer, policy, reference=False):
    """Search all contextual unit assignments and all cell-break groupings."""
    text='\n'.join(physical_lines(answer,policy))
    unique=tuple(sorted(set(units),key=repr)); counter=Counter(units)
    initial=tuple(counter[u] for u in unique)
    by_key={}
    for index,u in enumerate(unique): by_key.setdefault((u.kind,u.value),[]).append(index)
    words=vocabulary(policy); labels=tuple(words[name] for name in policy['state_labels']['constants'])
    @lru_cache(maxsize=None)
    def options(offset): return match_options(text,offset,units)
    @lru_cache(maxsize=None)
    def visit(offset,remaining,context,extras):
        if offset==len(text):
            return () if all(not count or (not reference and u.kind in policy['optional_nonrows']) for u,count in zip(unique,remaining)) else None
        for key,end in options(offset):
            for index in by_key.get(key,()):
                if not remaining[index] or not compatible(unique[index],context): continue
                counts=list(remaining); counts[index]-=1
                next_offset=end+1 if end<len(text) else end
                rest=visit(next_offset,tuple(counts),next_context(unique[index],context),extras)
                if rest is not None: return ((unique[index],text[offset:end],context[0]),)+rest
        end=text.find('\n',offset)
        if end<0:end=len(text)
        raw=text[offset:end]
        if (not reference and extras<policy['commentary_max_lines'] and context[0] not in policy['required_parts']
                and not row_like(None,raw) and not any(label in raw for label in labels)):
            rest=visit(end+1 if end<len(text) else end,remaining,context,extras+1)
            if rest is not None:return ((None,raw,context[0]),)+rest
        return None
    return visit(0,initial,('','',()),0)


@lru_cache(maxsize=1024)
def reference_units(source, serialized):
    policy=json.loads(serialized); validate(policy)
    units=parse_reference(source,policy)
    # The same complete-assignment search validates reference coverage too.
    if find_assignment(units,source,policy,reference=True) is None:
        raise Unsupported('unassigned_reference_line')
    return units


def diagnostic_path(units,answer,policy):
    """Deterministic counts for a rejected answer; never accepts a result."""
    remaining=Counter(units); text='\n'.join(physical_lines(answer,policy)); context=('','',()); path=[]; offset=0
    while offset<len(text):
        found=None
        for key,end in match_options(text,offset,units):
            candidates=sorted((u for u,count in remaining.items() if count and (u.kind,u.value)==key and compatible(u,context)),key=repr)
            if candidates: found=(candidates[0],end); break
        if found:
            unit,end=found; remaining[unit]-=1; path.append((unit,text[offset:end],context[0])); context=next_context(unit,context)
        else:
            end=text.find('\n',offset)
            if end<0:end=len(text)
            path.append((None,text[offset:end],context[0]))
        offset=end+1 if end<len(text) else end
    return tuple(path)


def compare(units, answer, policy):
    path=find_assignment(units,answer,policy)
    accepted=path is not None
    if path is None:path=diagnostic_path(units,answer,policy)
    kept=Counter(u for u,raw,part in path if u is not None); remaining=Counter(units)-kept
    added=[(raw,part) for u,raw,part in path if u is None]; rules=set()
    relocated={key[1] for raw,part in added for key,end in match_options(raw,0,units) if key[0]=='row' and end==len(raw)}
    for unit,count in remaining.items():
        if unit.kind in policy['optional_nonrows']:continue
        rules.add((2 if unit.value in relocated else 1) if unit.kind=='row' else 3 if unit.kind in ('unknown','next','text') else 2)
    if any(row_like(None,raw) or part in policy['required_parts'] for raw,part in added):rules.add(4)
    words=vocabulary(policy); labels=tuple(words[name] for name in policy['state_labels']['constants'])
    if len(added)>policy['commentary_max_lines'] or any(label in raw for raw,part in added for label in labels):rules.add(5)
    nonrows={kind:{'total':sum(u.kind==kind for u in units),'kept':sum(n for u,n in kept.items() if u.kind==kind)} for kind in KINDS}
    return {'facts_kept':'pass' if accepted else 'FAIL','failed_rules':[] if accepted else sorted(rules),'fidelity':{
        'rows_total':sum(u.kind=='row' for u in units),'rows_kept':sum(n for u,n in kept.items() if u.kind=='row'),
        'cells_total':sum(len(u.value) for u in units if u.kind=='row'),
        'cells_kept':sum(len(u.value)*n for u,n in kept.items() if u.kind=='row'),
        'nonrows':nonrows,'added_lines':len(added),'added_chars':sum(len(raw) for raw,part in added)}}


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
