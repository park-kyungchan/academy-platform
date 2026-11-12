"""M24 request contracts and bounded replay on the same synthetic server clock."""
import json
import subprocess


def replay(binary, scenario, calls, today='2026-11-11'):
    academy = [c for c in calls if str(c['name']).startswith('mcp__academy__')]
    if len(academy) > 32:
        raise ValueError('replay_call_limit')
    if not academy:
        return []
    messages = [{'id': i, 'method': 'tools/call', 'params': {
        'name': c['name'].removeprefix('mcp__academy__'), 'arguments': json.loads(c['arguments'])}}
        for i, c in enumerate(academy)]
    proc = subprocess.run([str(binary), '--scenario', str(scenario), '--today', today],
        input='\n'.join(json.dumps(m, ensure_ascii=False) for m in messages)+'\n',
        text=True, capture_output=True, timeout=30, check=True)
    if len(proc.stdout.encode()) > 2_000_000:
        raise ValueError('replay_bytes_limit')
    responses = [json.loads(line) for line in proc.stdout.splitlines()]
    if len(responses) != len(messages) or any(r.get('id') != i for i, r in enumerate(responses)):
        raise ValueError('replay_response_order')
    return [r.get('result', {}).get('_meta', {}).get('academy/envelope') for r in responses]


def evaluate(spec, calls, envelopes, previous):
    academy = [c for c in calls if str(c['name']).startswith('mcp__academy__')]
    required = spec['required_arguments']
    checks = {'expected_tool': 'pass' if academy and all(c['name'] == 'mcp__academy__'+spec['expected_tool'] for c in academy) else 'FAIL',
              'call_order': 'pass' if len(academy) == 1 else 'FAIL'}
    if not academy:
        checks.update(required_arguments='FAIL', server_replay='unknown')
        if required.get('activity'):
            checks['activity_scope'] = 'FAIL'
        return checks
    arguments = json.loads(academy[-1]['arguments'])
    valid = isinstance(arguments, dict)
    student = arguments.get('student') if valid else None
    valid = valid and student in (required['student'], required['student']+' (가상반A)')
    valid = valid and arguments.get('topic') == required['topic']
    if required.get('book_explicit'):
        valid = valid and isinstance(arguments.get('book'), str) and bool(arguments['book'].strip())
    if required.get('ask'):
        valid = valid and not any(arguments.get(k) for k in ('book','unit','unit_from','unit_to'))
    envelope = envelopes[-1] if envelopes else None
    scope = 'unknown'
    if envelope is not None:
        if required.get('ask'):
            scope = 'pass' if (envelope.get('ask') or {}).get('field') == required['ask'] else 'FAIL'
        else:
            data = envelope.get('data', {})
            minors = [leaf['minor']['code'] for leaf in data.get('minors', [])]
            ok = (envelope.get('status') == 'ok' and data.get('student') == required['student']
                  and data.get('edition') == required['edition'] and data.get('topic') == required['topic']
                  and minors == required['minors'] and data.get('level') == required.get('level')
                  and data.get('activity') == required.get('activity'))
            scope = 'pass' if ok else 'FAIL'
    checks['required_arguments'] = 'FAIL' if not valid else scope
    if required.get('activity'):
        unit = arguments.get('unit') if isinstance(arguments, dict) else None
        checks['activity_scope'] = 'pass' if isinstance(unit, str) and required['activity'] in unit else 'FAIL'
    if spec.get('followup'):
        ok = previous is not None and (not required.get('book_explicit') or
             (previous.get('ask') or {}).get('field') == 'book')
        checks['call_order'] = 'pass' if checks['call_order'] == 'pass' and ok else 'FAIL'
    checks['server_replay'] = ('unknown' if envelopes is None or len(envelopes) != len(academy) else
        'pass' if all(e is not None and c.get('received_display') == e.get('display')
                      for c, e in zip(academy, envelopes)) else 'FAIL')
    return checks
