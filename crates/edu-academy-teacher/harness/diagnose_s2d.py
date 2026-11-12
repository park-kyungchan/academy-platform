#!/usr/bin/env python3
"""Re-score retained S2d2 evidence; export only aggregate diagnostic tables."""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

import acceptance
import diffclass as d
import fidelity as f
import request_checks as checks
import run

FIELDS = {'student','topic','book','unit','unit_from','unit_to'}
WORD = {'level':'기본','activity':'필수예제'}
SOURCE = {
    'level': 'src/unit.rs:213-224,330-338',
    'activity': 'src/unit.rs:322-329',
    'book': 'src/unit.rs:174-210',
    'minors': 'src/unit.rs:339-415',
    'student': 'src/unit.rs:145-148',
    'topic': 'src/unit.rs:82-137',
}


def load(path, limit=d.MAX_METADATA_BYTES):
    return json.loads(d.read_bounded(path, limit))


def retained(row):
    matches=[]
    pattern=f"p8-s2d2-{row['mode']}-{row['request']}-{row['conversation']}-*"
    roots=list(Path(tempfile.gettempdir()).glob(pattern))
    if len(roots)>d.MAX_CANDIDATES: raise ValueError('candidate_limit')
    for root in roots:
        directory=root/'run'
        saved=load(directory/'measurement.json')
        if (saved.get('request'),saved.get('mode'),saved.get('conversation')) != (
                row['request'],row['mode'],row['conversation']): continue
        if d.saved_fingerprint(saved['results'][row['turn']-1])==d.row_fingerprint(row):
            matches.append((directory,saved))
    if len(matches)!=1: raise ValueError('retained_evidence_not_unique')
    return matches[0]


def locate_word(args, word):
    return tuple(sorted(k if k in FIELDS else 'other' for k,v in args.items()
                        if isinstance(v,str) and word in v))


def wrong_elements(spec, args, envelope):
    required=spec['required_arguments']; data=(envelope or {}).get('data',{})
    if required.get('ask'): data=(envelope or {}).get('_book_scope',{})
    actual=dict(data,book=data.get('edition'),minors=[x['minor']['code'] for x in data.get('minors',[])])
    expected={k:required.get(k) for k in ('student','topic','level','activity')}
    if 'edition' in required: expected.update(book=required['edition'],minors=required['minors'])
    if (envelope or {}).get('status')!='ok' and not required.get('ask'):
        # An early ask/stop has no resolved data: that is not evidence that
        # student, topic and every downstream field had the wrong value.
        field=((envelope or {}).get('ask') or {}).get('field')
        code=((envelope or {}).get('stop') or {}).get('code')
        wrong=[{'book':'book','unit':'minors','unit_from':'minors','unit_to':'minors',
                'student':'student','topic':'topic'}.get(field,
                'book' if code=='book_not_found' else 'student' if code=='student_not_found' else 'minors')]
        if required.get('level') and 'level' not in wrong: wrong.append('level')
        if required.get('activity') and 'activity' not in wrong: wrong.append('activity')
        book=(envelope or {}).get('_diagnostic_book',{})
        if book.get('edition') and book['edition']!=required.get('edition') and 'book' not in wrong:
            wrong.append('book')
    else:
        wrong=[k for k,v in expected.items() if actual.get(k)!=v]
    if required.get('book_explicit') and not args.get('book') and 'book' not in wrong: wrong.append('book')
    result=[]
    for element in wrong:
        word=WORD.get(element)
        locations=locate_word(args,word) if word else ()
        if word and 'unit' in locations: how='present_in_unit_not_resolved'
        elif word and locations: how='in_other_argument:'+','.join(locations)
        elif word or (element=='book' and not args.get('book')): how='absent_everywhere'
        else: how='wrong_value'
        result.append((element,how))
    return result


def line_kind(line, index, ask=False):
    """Physical-line roles from DISPLAY-SPEC-S2, not M21 logical unit kinds."""
    if ask: return 'choice' if line.startswith('- ') else 'question' if line.endswith('？') or line.endswith('?') else 'missing_fact'
    if index==0: return 'result_title'
    if line.startswith('읽은 원천'): return 'source_footer'
    if line.startswith(('학생·교재 등록','교사 사실')): return 'source_footer_continuation'
    if line.startswith('다음 확인:'): return 'next_check'
    if line.startswith('이번 결과는'): return 'read_only_closing'
    if line.startswith('—:'): return 'dash_legend'
    if ' | ' in line: return 'grid_header' if line.startswith('소단원') else 'unit_grid_row'
    if '대상 근거:' in line: return 'clinic_eligibility_basis'
    if '교사가 제외' in line: return 'exclusion'
    if '교사 판정:' in line: return 'teacher_judgment'
    if line.startswith(('오답클리닉 풀이:','오답클리닉 채점:','유사클리닉:')): return 'clinic_stage'
    if '섹션:' in line: return 'source_absence'
    if '시험 날짜:' in line or '시험 수준:' in line: return 'source_qualification'
    if '원천:' in line: return 'source_observation'
    return 'source_detail'


def line_key(line, policy):
    fields=f.fields_of(line,policy)
    if fields: return tuple(f.value(v,policy) for v in fields)
    return f.statement(f.heading(line,policy),policy)


def display_diff(source, answer, policy):
    refs=f.physical_lines(source,policy,True); lines=f.physical_lines(answer,policy)
    remaining=Counter(line_key(line,policy) for line in lines)
    missing=Counter()
    for i,line in enumerate(refs):
        key=line_key(line,policy)
        if remaining[key]: remaining[key]-=1; continue
        category=d.missing_class(line,lines,refs[:i]+refs[i+1:],d.Budget())
        missing[(line_kind(line,i),'lost' if category=='absent' else 'changed',category)]+=1
    available=Counter(line_key(line,policy) for line in refs); matched=[]; added=[]
    for i,line in enumerate(lines):
        key=line_key(line,policy)
        if available[key]: available[key]-=1; matched.append(i)
        else: added.append(i)
    positions=Counter('unanchored' if not matched else 'before' if i<min(matched)
                      else 'after' if i>max(matched) else 'between' for i in added)
    return missing,positions


def write_rows(path, rows):
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    with os.fdopen(fd,'w') as stream:
        for row in rows: stream.write(json.dumps(row,ensure_ascii=True,sort_keys=True)+'\n')


def analyze(rows):
    specs=load(run.HERE/'checks.json'); rescored=[]; changed=[]; evidence=[]
    grouped=defaultdict(list)
    for row in rows:
        d.identity(row)
        grouped[(row['mode'],row['request'],row['conversation'])].append(row)
    if len(rows)!=120 or len(grouped)!=90: raise ValueError('incomplete_matrix')
    if hashlib.sha256(acceptance.BINARY.read_bytes()).hexdigest()!='a81ae6c1ff4e01e3a856cf2614532554a6e392b35ca0307fc6c8ff7cd4a8f028':
        raise ValueError('frozen_binary_mismatch')
    for (_,q,n), old in grouped.items():
        old.sort(key=lambda r:r['turn']); directory,saved=retained(old[0])
        if len(old)!=len(specs[q]['turns']): raise ValueError('incomplete_conversation')
        raw=d.read_bounded(directory/'claude.jsonl',d.MAX_STREAM_BYTES)
        if len(raw.splitlines())>d.MAX_EVENTS: raise ValueError('stream_event_limit')
        calls=run.measure([json.loads(line) for line in raw.splitlines()])['calls']
        replayed=checks.replay(acceptance.BINARY,acceptance.SCENARIO,calls)
        allowed=['mcp__academy__'+t['name'] for t in load(run.CRATE/'tests/fixtures/tools-list.json')['tools']]
        log=d.read_bounded(directory/'run-log.jsonl',d.MAX_LOG_BYTES).splitlines()
        results=run.score_turns(raw,specs[q]['turns'],allowed,saved['mode'],[json.loads(v) for v in log],
                                saved['error'],saved['sent_turns'],replayed)
        offset=0
        for prior,result,spec in zip(old,results,specs[q]['turns']):
            if d.saved_fingerprint(result)!=d.row_fingerprint(prior): raise ValueError('retained_fingerprint_mismatch')
            new=dict(prior,**run.public_row(result,q,'haiku',saved['mode'],n))
            new['request_checks_passed']=not new['failed_request_checks']
            rescored.append(new)
            if new['result']!=prior['result']:
                changed.append(dict(new,previous_result=prior['result'],previous_failed_request_checks=prior['failed_request_checks']))
            academy=[c for c in result['calls'] if c['name'].startswith('mcp__academy__')]
            envs=replayed[offset:offset+len(academy)]; offset+=len(academy)
            evidence.append((new,spec,result,academy,envs))
    a=Counter(); b=Counter(); shapes=Counter(); losses=Counter(); positions=Counter(); profiles=Counter()
    failed_args=[]; failed_displays=[]
    for row,spec,result,calls,envs in evidence:
        key=(row['request'],row['turn'],row['mode'])
        args=json.loads(calls[-1]['arguments']) if calls else {}
        if 'required_arguments' in row['failed_request_checks']:
            env=envs[-1] if envs else {}
            if (env.get('ask') or {}).get('field')=='unit' and args.get('book'):
                probe={k:v for k,v in args.items() if k in ('student','book','topic')}
                response=checks.replay(acceptance.BINARY,acceptance.SCENARIO,
                    [dict(calls[-1],arguments=json.dumps(probe))])[0]
                env['_diagnostic_book']=(response or {}).get('data',{})
            wrong=wrong_elements(spec,args,envs[-1] if envs else None)
            if not wrong: raise ValueError('unclassified_required_argument_failure')
            failed_args.append(dict(request=key[0],turn=key[1],mode=key[2],conversation=row['conversation'],
                                    wrong_elements=[dict(element=k,how=v) for k,v in wrong]))
            for element,how in wrong:
                a[key+(element,how)]+=1
                if element=='book' and env.get('_diagnostic_book',{}).get('edition'):
                    behavior='resolves_wrong_book_then_asks_unit'
                elif env.get('ask'): behavior='refuses_with_question:'+env['ask']['field']
                elif env.get('stop'): behavior='refuses_with_message'
                elif how.startswith('in_other_argument:') or how=='present_in_unit_not_resolved': behavior='ignores_silently'
                elif how=='absent_everywhere': behavior='resolves_without_missing_qualifier'
                else: behavior='resolves_explicit_wrong_value'
                b[key+(element,how,behavior)]+=1
        if row['request'] in ('Q12','Q15'):
            fields=tuple(sorted(k if k in FIELDS else 'other' for k in args))
            shapes[key+(fields,locate_word(args,WORD['level']),locate_word(args,WORD['activity']))]+=1
        if row['request'] in ('Q11','Q13','Q14','Q16'):
            source=result['display']; lines=source.splitlines()
            kinds=tuple(sorted(Counter(line_kind(line,i,row['request']=='Q14' and row['turn']==1) for i,line in enumerate(lines)).items()))
            profiles[key+(len(lines),len(source.encode()),kinds)]+=1
        if (row['request'],row['turn']) in (('Q13',1),('Q16',1),('Q12',2)) and row['facts_kept']!='pass':
            missing,added=display_diff(result['display'],result['answer'],spec['fidelity_policy'])
            failed_displays.append(dict(request=key[0],turn=key[1],mode=key[2],conversation=row['conversation'],
                losses=[dict(line_kind=k,change=v,classification=c,count=n) for (k,v,c),n in sorted(missing.items())],
                added_positions=dict(added)))
            for (kind,change,category),count in missing.items(): losses[key+(kind,change,category)]+=count
            for position,count in added.items(): positions[key+(position,)]+=count
    tables={'A':{'failed_turns':failed_args,'wrong_elements':[dict(zip(('request','turn','mode','element','how','count'),key+(count,))) for key,count in sorted(a.items())],
                 'all_call_shapes':[dict(zip(('request','turn','mode','arguments','level_locations','activity_locations','count'),key+(count,))) for key,count in sorted(shapes.items())]},
            'B':[], 'C':{'failed_turns':failed_displays,'losses':[dict(zip(('request','turn','mode','line_kind','change','class','count'),key+(count,))) for key,count in sorted(losses.items())],
                         'added_positions':[dict(zip(('request','turn','mode','position','count'),key+(count,))) for key,count in sorted(positions.items())],
                         'display_profiles':[dict(zip(('request','turn','mode','lines','bytes','line_kinds','count'),key+(count,))) for key,count in sorted(profiles.items())]},'D':[]}
    for (q,t,mode,element,how,behavior),count in sorted(b.items()):
        source=('src/unit.rs:174-210,339-398' if behavior=='resolves_wrong_book_then_asks_unit' else
                'src/unit.rs:339-398' if behavior=='refuses_with_question:unit' else
                'src/unit.rs:174-210' if behavior=='refuses_with_message' else SOURCE[element])
        tables['B'].append(dict(request=q,turn=t,mode=mode,element=element,form=how,behavior=behavior,
                                count=count,source=source))
    tables['D']=candidates()
    return rescored,changed,tables


def candidates():
    # Byte deltas are proposals computed from the advertised JSON, never applied.
    tools=load(run.CRATE/'tests/fixtures/tools-list.json')
    schema=next(t['inputSchema'] for t in tools['tools'] if t['name']=='academy_unit_status')
    compact=lambda value: json.dumps(value,ensure_ascii=False,separators=(',',':')).encode()
    level={'type':'string','description':'시험 수준','enum':['기본','발전']}
    level_bytes=len(compact(dict(schema['properties'],level=level)))-len(compact(schema['properties']))
    descriptions={k:dict(v) for k,v in schema['properties'].items()}
    descriptions['unit']['description']+=' 시험 수준(기본·발전)도 함께 넣습니다.'
    descriptions['topic']['description']='항목. 섹션 점수·오답은 섹션, 모든 항목을 요청한 경우만 전체.'
    description_bytes=len(compact(descriptions))-len(compact(schema['properties']))
    return [dict(failing_class='arguments',candidate=1,
                 change='Add optional typed level enum with a short field label; apply it alongside legacy unit filters, reject conflicts.',
                 contract='M24(d/e); UnitStatusRequest/tools-list budget; missing level still means all levels',
                 tools_list_bytes_added=level_bytes,
                 reading='Fresh 10 isolated/5 installed conversations per request: requested qualifiers supplied, Q12/Q15 checks >=9/10 isolated, no fidelity regression.'),
            dict(failing_class='arguments',candidate=2,
                 change='Keep the activity clause; add the short level-in-unit and narrow-topic descriptions declared in this script.',
                 contract='M24(d/e), tools-list budget',tools_list_bytes_added=description_bytes,
                 reading='Same new request matrix; count omitted levels, explicit broader topics and resulting scope checks separately.'),
            dict(failing_class='display',candidate=1,
                 change='Renderer encloses the entire display in one text code fence, inside the unchanged two-block carrier.',
                 contract='M21 allowed outer fence; M24(a/c); DISPLAY-SPEC-S2 fixtures/byte cap',tools_list_bytes_added=0,
                 reading='New matrix with fenced display as reference; Q13.1/Q16.1/Q12.2 facts_kept >=9/10, all physical line-kind losses and additions counted.'),
            dict(failing_class='display',candidate=2,
                 change='Renderer gives clinic details explicit group headings and small scopes a uniform two-column fact table; preserve every fact and source qualification.',
                 contract='DISPLAY-SPEC-S2 composition; M21 binding; M22(a/b/e), M23(a/h)',tools_list_bytes_added=0,
                 reading='New matrix plus parser coverage tests; compare the same line-kind loss and position tables, without relaxing M21.')]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('rows',type=Path); parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--changed',type=Path,required=True); parser.add_argument('--tables',type=Path,required=True)
    args=parser.parse_args()
    try:
        rows=[json.loads(line) for line in d.read_bounded(args.rows,d.MAX_STREAM_BYTES).splitlines()]
        rescored,changed,tables=analyze(rows)
        write_rows(args.output,rescored); write_rows(args.changed,changed); write_rows(args.tables,[tables])
    except (OSError,ValueError,KeyError,TypeError,IndexError,AssertionError,UnicodeError,
            AttributeError,RecursionError,subprocess.SubprocessError):
        print('diagnosis_failed@diagnose_s2d.py:0: retained synthetic evidence unavailable',file=sys.stderr)
        return 2
    print(json.dumps(tables,ensure_ascii=True,sort_keys=True))
    return 0


if __name__=='__main__':
    sys.exit(main())
