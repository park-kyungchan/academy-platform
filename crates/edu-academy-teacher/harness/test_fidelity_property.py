"""Generated and public-fixture M21 properties; fixed seeds, standard library only."""
from collections import defaultdict
import json
import random
import re
import time
import unittest
import fidelity

CASES=300


def metadata(source):
    """Independent fixture inventory; never calls the production reference parser."""
    lines=source.splitlines(); rows=[]; statements=[]; headers=[]; parts=[]; students=[]; footer=[]; closing=[]
    part=student=''; header=(); in_footer=False
    for i,line in enumerate(lines):
        clean=re.sub(r'^#{1,6} ', '',line).strip('*').strip()
        if clean.startswith('읽은 원천') or in_footer:
            if clean=='이번 결과는 읽기만 했고 저장한 것은 없습니다.':
                closing.append(i);in_footer=False
            else:footer.append(i);in_footer=True
            continue
        if clean=='이번 결과는 읽기만 했고 저장한 것은 없습니다.':closing.append(i);continue
        if line.startswith('## ') or clean.endswith(' 학원 내 처리'):
            student=clean;students.append((i,clean));continue
        if line.startswith('# ') or clean in ('알림','학생별 할 일','미확인 사항','다음 확인'):
            part=clean;student='';header=();parts.append((i,clean));continue
        fields=re.split(r'\s+[|｜]\s+',line.strip().strip('|｜').strip())
        if len(fields)>1:
            if not header or tuple(fields)==header:
                header=tuple(fields);headers.append(i)
            else:rows.append((i,line,fields,(part,student,header)))
        elif i:statements.append((i,line,part))
    groups=defaultdict(list)
    for row in rows:groups[row[3]].append(row)
    return dict(lines=lines,rows=rows,statements=statements,headers=headers,parts=parts,students=students,footer=footer,closing=closing,groups=groups)


def generated(seed, labels):
    rng=random.Random(seed); lines=['Title '+str(seed)]
    tableless=seed%5==0
    for p in range(1+seed%3):
        lines.append('# Part '+str(p))
        if tableless:
            lines.append('Read chapter '+str(p));continue
        for student in range(max(1,seed%4)):
            if seed%4:lines.append('## Student '+str(student))
            n=2+(seed+p)%5
            lines.append('| '+' | '.join('Field '+str(c) for c in range(n))+' |')
            count=rng.randrange(7)
            base=[['Alice','2026-11-01','A·B','3/5','X｜Y','42'][c] for c in range(n)]
            column=(seed+p)%n
            if seed%3==0 and column!=n-1:base[column]+=' / detail'
            if seed%7==0 and n>=3:
                base[0]+=' / first';base[1]+=' / second'
            for row in range(count):
                cells=list(base)
                cells[-1]='Review' if row%3==0 else 'Review / book'
                lines.append('| '+' | '.join(cells)+' |')
    for j,name in enumerate(('미확인 사항','다음 확인')):
        lines.append(name)
        if not tableless and seed%4==1:
            lines.append('| Item | State |')
            lines.append('| Shared | Review |')
            lines.append('| Shared | Review / book |')
        for number in range((seed+j)%4):lines.append(labels[(seed+number)%len(labels)]+' fact '+str(number))
    if seed%2:lines.append('읽은 원천 Fixture '+str(seed))
    if seed%3:lines.append('이번 결과는 읽기만 했고 저장한 것은 없습니다.')
    return '\n'.join(lines)


def reorderings(groups):
    choices=[]
    for group in groups:
        original=[row[1] for row in group]
        changed=original[1:]+original[:1]
        if changed!=original:choices.append((group,changed))
    return choices


class GeneratedFidelity(unittest.TestCase):
    def exercise_state_vocabulary(self,policy):
        words=fidelity.vocabulary(policy); names=policy['state_labels']['constants']; labels=[words[name] for name in names]
        opposites=set(); additions=set()
        for name,label in zip(names,labels):
            source='State fixture\n미확인 사항\n'+label+'\n다음 확인'
            self.assertEqual(fidelity.evaluate(source,source,policy)['facts_kept'],'pass')
            self.assertEqual(fidelity.evaluate(source,source.replace(label,label+' 아님',1),policy)['facts_kept'],'FAIL');opposites.add(name)
            self.assertEqual(fidelity.evaluate(source,label+'\n'+source,policy)['facts_kept'],'FAIL');additions.add(name)
        self.assertEqual(opposites,set(names));self.assertEqual(additions,set(names))
        return 3*len(names)

    def test_every_state_word_opposite_and_added_label(self):
        self.exercise_state_vocabulary(fidelity.configured_policy())

    def test_generated_and_public_references(self):
        start=time.perf_counter();policy=fidelity.configured_policy(); words=fidelity.vocabulary(policy)
        names=policy['state_labels']['constants']; labels=[words[name] for name in names]
        references=[('G'+str(seed),generated(seed,labels)) for seed in range(CASES)]
        fixtures=sorted((fidelity.CRATE/'tests/fixtures').glob('display-*.txt'))
        references += [('F:'+path.name,path.read_text()) for path in fixtures]
        applications=defaultdict(set); eligible=defaultdict(set); assertions=0
        columns=set(); equal_rows=prefix_rows=break_free=tableless=0
        def check(identifier,category,source,answer,expected):
            nonlocal assertions
            result=fidelity.evaluate(source,answer,policy)
            self.assertNotIn(result.get('reason_code'),('reference_too_large','answer_too_large','search_budget'),(identifier,category))
            self.assertEqual(result['facts_kept'],expected,(identifier,category))
            applications[category].add(identifier); assertions+=1
        def join(lines,index,replacement):return '\n'.join(lines[:index]+replacement+lines[index+1:])
        for sequence,(identifier,source) in enumerate(references):
            m=metadata(source); lines=m['lines']; rows=m['rows']; statements=m['statements']
            if not m['headers']:tableless+=1
            for row in rows:
                columns.add(len(row[2]));break_free+=any(' / ' not in cell for cell in row[2])
            values=[tuple(r[2]) for r in rows]
            equal_rows+=len(values)!=len(set(values))
            prefix_rows+=any(a!=b and all(y.startswith(x) for x,y in zip(a,b)) for a in values for b in values)
            breakable=[(row,c) for row in rows for c,cell in enumerate(row[2]) if ' / ' in cell]
            reorder=reorderings(m['groups'].values())
            multi=[row for row in rows if sum(' / ' in cell for cell in row[2])>1]
            move=[(row,target) for row in rows for target,name in m['parts'] if name!=row[3][0]]
            move_student=[(row,target) for row in rows for target,name in m['students'] if name!=row[3][1]]
            applicable={'identity':True,'drop_row':bool(rows),'change_field':bool(rows),'duplicate_row':bool(rows),
                'move_context':bool(move),'move_student':bool(move_student),'drop_statement':bool(statements),
                'opposite_statement':bool(statements),'invented_row':True,'two_comments':True,'state_comment':True,
                'required_addition':any(name in policy['required_parts'] for _,name in m['parts']),
                'one_comment':True,'reorder_rows':bool(reorder),'cell_break':bool(breakable),'cell_break_reverse':bool(breakable),
                'multi_cell_break':bool(multi),'multi_cell_break_reverse':bool(multi),'full_width_delimiters':bool(m['headers']),'outer_fence':True,'separator':bool(m['headers']),
                'drop_footer':bool(m['footer']),'drop_closing':bool(m['closing'])}
            for category,applies in applicable.items():
                if applies:eligible[category].add(identifier)
            check(identifier,'identity',source,source,'pass')
            if rows:
                index,text,cells,binding=rows[0]
                check(identifier,'drop_row',source,join(lines,index,[]),'FAIL')
                check(identifier,'change_field',source,join(lines,index,[text.replace(cells[0],cells[0]+' changed',1)]),'FAIL')
                check(identifier,'duplicate_row',source,join(lines,index,[text,text]),'FAIL')
            for category,possibilities in [('move_context',move),('move_student',move_student)]:
                if possibilities:
                    row,target=possibilities[0]; index,text,_,_=row
                    changed=lines[:index]+lines[index+1:];target=target+1-(target>index);changed.insert(target,text)
                    check(identifier,category,source,'\n'.join(changed),'FAIL')
            if statements:
                index,text,_=statements[0]
                check(identifier,'drop_statement',source,join(lines,index,[]),'FAIL')
                label=labels[sequence%len(labels)]
                # Negated vocabulary label is a complete replacement, never a substring edit.
                check(identifier,'opposite_statement',source,join(lines,index,[label+' 아님']),'FAIL')
            check(identifier,'invented_row',source,'| New | Invented |\n'+source,'FAIL')
            check(identifier,'two_comments',source,'# First comment\nOther 학원 내 처리\n'+source,'FAIL')
            check(identifier,'state_comment',source,labels[sequence%len(labels)]+'\n'+source,'FAIL')
            required=[i for i,name in m['parts'] if name in policy['required_parts']]
            if required:
                index=required[0]
                check(identifier,'required_addition',source,'\n'.join(lines[:index+1]+['Additional factual line']+lines[index+1:]),'FAIL')
            check(identifier,'one_comment',source,'One stateless comment\n'+source,'pass')
            if reorder:
                group,reordered=reorder[0]; changed=list(lines)
                self.assertNotEqual([row[1] for row in group],reordered)
                for row,text in zip(group,reordered):changed[row[0]]=text
                check(identifier,'reorder_rows',source,'\n'.join(changed),'pass')
            if breakable:
                row,column=breakable[sequence%len(breakable)];index,text,cells,_=row
                wrapped=join(lines,index,[text.replace(cells[column],cells[column].replace(' / ','\n'),1)])
                check(identifier,'cell_break',source,wrapped,'pass')
                check(identifier,'cell_break_reverse',wrapped,source,'pass')
            if multi:
                index,text,cells,_=multi[0]
                wrapped=join(lines,index,[text.replace(' / ','\n')])
                check(identifier,'multi_cell_break',source,wrapped,'pass')
                check(identifier,'multi_cell_break_reverse',wrapped,source,'pass')
            if m['headers']:
                check(identifier,'full_width_delimiters',source,source.replace(' | ',' ｜ '),'pass')
                index=m['headers'][0];n=len(re.split(r'\s+[|｜]\s+',lines[index].strip().strip('|｜').strip()))
                check(identifier,'separator',source,join(lines,index,[lines[index],'| '+' | '.join(['---']*n)+' |']),'pass')
            check(identifier,'outer_fence',source,'```text\n'+source+'\n```','pass')
            if m['footer']:check(identifier,'drop_footer',source,'\n'.join(line for i,line in enumerate(lines) if i not in m['footer']),'pass')
            if m['closing']:check(identifier,'drop_closing',source,'\n'.join(line for i,line in enumerate(lines) if i not in m['closing']),'pass')
        assertions+=self.exercise_state_vocabulary(policy)
        self.assertEqual(set(applications),set(eligible))
        for category,expected in eligible.items():self.assertEqual(applications[category],expected,category);self.assertTrue(expected,category)
        self.assertTrue({2,3,4,5,6}.issubset(columns));self.assertGreater(tableless,0);self.assertGreater(equal_rows,0);self.assertGreater(prefix_rows,0);self.assertGreater(break_free,0)
        elapsed=time.perf_counter()-start
        self.assertLess(elapsed,20.0)
        print('property_summary='+json.dumps({'generated':CASES,'fixtures':len(fixtures),'assertions':assertions,'seconds':round(elapsed,3),'state_labels':len(names),'limit_hits':0,'applications':{key:len(value) for key,value in sorted(applications.items())}},sort_keys=True))
