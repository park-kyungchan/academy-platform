#!/usr/bin/env python3
"""S2d acceptance: qualified frozen server, exact-call replay, counts-only export."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import diffclass
import request_checks
import run

PRIVATE = run.CRATE/'.private/s2d2'
BINARY = PRIVATE/'edu-academy-teacher'
SCENARIO = run.CRATE/'tests/fixtures/scenario-v3.json'
REQUESTS = tuple('Q'+str(i) for i in range(11,17))


def save(path, value):
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    with os.fdopen(fd,'w') as stream:
        json.dump(value,stream,ensure_ascii=False)


def prepare(source):
    PRIVATE.mkdir(mode=0o700,parents=True,exist_ok=True)
    if BINARY.exists(): raise ValueError('binary_already_frozen')
    shutil.copy2(source,BINARY); BINARY.chmod(0o700)
    print(json.dumps({'binary_sha256':hashlib.sha256(BINARY.read_bytes()).hexdigest()}),flush=True)


def qualify():
    specs=json.loads((run.HERE/'checks.json').read_text())
    for q in REQUESTS:
        calls=[]
        for spec in specs[q]['turns']:
            r=spec['required_arguments']; args={'student':r['student'],'topic':r['topic']}
            if r.get('book_explicit'): args['book']=r['edition']
            if len(r.get('minors',[]))==1: args['unit']=r['minors'][0]+' '+(r.get('activity') or r.get('level') or '')
            calls.append({'name':'mcp__academy__academy_unit_status','arguments':json.dumps(args)})
        responses=request_checks.replay(BINARY,SCENARIO,calls)
        for i,(call,response,spec) in enumerate(zip(calls,responses,specs[q]['turns'])):
            call['received_display']=response['display']
            assert all(v=='pass' for v in request_checks.evaluate(spec,[call],[response],responses[i-1] if i else None).values()),q
            if q=='Q12':
                path=run.CRATE/'tests/fixtures'/spec['fixture']
                with path.open('x') as output: output.write(response['display'])
        save(PRIVATE/(q+'-references.json'),responses)
    print(json.dumps({'binary_sha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),'qualified_turns':8}),flush=True)


def plan():
    return [(mode,q,n) for mode,total in (('isolated',10),('installed',5)) for n in range(1,total+1) for q in REQUESTS]


def conversation(mode,q,n):
    os.umask(0o077)
    specs=json.loads((run.HERE/'checks.json').read_text())[q]['turns']
    allowed=['mcp__academy__'+t['name'] for t in json.loads((run.CRATE/'tests/fixtures/tools-list.json').read_text())['tools']]
    root=Path(tempfile.mkdtemp(prefix='p8-s2d2-'+mode+'-'+q+'-'+str(n)+'-'))
    evidence,cwd=root/'run',root/'cwd'; evidence.mkdir(mode=0o700); cwd.mkdir(mode=0o700)
    save(evidence/'mcp.json',{'mcpServers':{'academy':{'command':str(BINARY),'args':[
        '--scenario',str(SCENARIO),'--today','2026-11-11','--run-log',str(evidence/'run-log.jsonl')]}}})
    command=run.command('claude','haiku',evidence/'mcp.json',','.join(allowed),mode)
    raw,exit_code,error,sent=run.drive_turns(command,specs,cwd,evidence,mode=mode)
    (evidence/'claude.jsonl').write_text(raw)
    calls=run.measure([json.loads(line) for line in raw.splitlines() if run._valid_json(line)])['calls']
    try: replayed=request_checks.replay(BINARY,SCENARIO,calls)
    except (OSError,ValueError,subprocess.SubprocessError): replayed=None
    results=run.score_turns(raw,specs,allowed,mode,run.read_call_log(evidence/'run-log.jsonl'),error,sent,replayed)
    save(evidence/'measurement.json',{'request':q,'model':'haiku','mode':mode,'conversation':n,'trial':False,
        'command':command,'sent_turns':sent,'error':error,'exit_code':exit_code,'results':results})
    save(evidence/'replay.json',replayed)
    rows=[]
    offset=0
    for result,spec in zip(results,specs):
        if not result['turn_sent']: continue
        row=run.public_row(result,q,'haiku',mode,n)
        row['request_checks_passed']=not row['failed_request_checks']
        row['directive_echo_count']=run.norm(result['answer'] or '').count(run.norm(run.DISPLAY_DIRECTIVE))
        count=sum(str(c['name']).startswith('mcp__academy__') for c in result['calls'])
        envelopes=replayed[offset:offset+count] if replayed is not None else []
        offset+=count
        kind='ask' if envelopes and envelopes[-1] and envelopes[-1].get('status')=='ask' else 'result'
        row['difference']=diffclass.analyze(result['display'],result['answer'],spec['fidelity_policy'],kind=kind)
        rows.append(row)
    return rows,error


def matrix(output):
    if output.read_bytes()!=b'': raise ValueError('measurement_output_not_empty')
    backup=output.with_name('S2d-runs.before-s2d2.jsonl')
    fd=os.open(backup,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600); os.close(fd)
    assert backup.read_bytes()==output.read_bytes()
    with output.open('a') as destination:
        for mode,q,n in plan():
            rows,error=conversation(mode,q,n)
            for row in rows: destination.write(json.dumps(row,ensure_ascii=False)+'\n')
            destination.flush()
            print(json.dumps({'mode':mode,'request':q,'conversation':n,'sent':len(rows),
                'kept':sum(r['facts_kept']=='pass' for r in rows),'checks':sum(r['request_checks_passed'] for r in rows),
                'first_request_tokens':rows[0]['first_request_tokens'] if rows else None,'driver_reason':error}),flush=True)
            if error=='first_request_tokens_not_under_6000': return
    print(json.dumps({'complete':True,'conversations':90}),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    group=parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--prepare',type=Path); group.add_argument('--matrix',type=Path)
    group.add_argument('--qualify',action='store_true')
    args=parser.parse_args()
    try:
        if args.prepare: prepare(args.prepare)
        elif args.qualify: qualify()
        else: matrix(args.matrix)
    except (OSError,ValueError,AssertionError):
        print('acceptance_failed@acceptance.py:0: invalid synthetic evidence',file=sys.stderr)
        raise SystemExit(2)
