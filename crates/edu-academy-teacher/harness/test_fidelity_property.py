"""Fixed-seed supported-design properties; no model, network or dependencies."""
import random
import time
import unittest
import fidelity

CASES=300
OPPOSITES={'제출 필요':'제출 필요 없음','Check book':'Do not check book'}


class GeneratedFidelity(unittest.TestCase):
    def test_300_supported_designs_and_mutations(self):
        start=time.perf_counter(); policy=fidelity.configured_policy(); checked=0; counts={}
        coverage={k:set() for k in ['parts','students','columns','rows','statements']}
        def check(seed, label, source, answer, expected):
            nonlocal checked
            actual=fidelity.evaluate(source,answer,policy)['facts_kept']
            self.assertEqual(actual,expected,(seed,label))
            checked+=1;counts[label]=counts.get(label,0)+1
        for seed in range(CASES):
            rng=random.Random(seed)
            lines=['Title '+str(seed)]; rows=[]; statements=[]; tables=0
            parts=rng.randint(1,3); coverage['parts'].add(parts)
            for part in range(parts):
                lines.append('# Part '+str(part)); n=rng.randint(2,5)
                students=rng.randint(0,3); coverage['students'].add(students); coverage['columns'].add(n)
                for student in range(max(1,students)):
                    if students: lines.append('## Student '+str(student))
                    lines.append('| '+' | '.join('Field '+str(c) for c in range(n))+' |')
                    # Exercise sparse and empty tables alongside two populated contexts.
                    row_count=rng.randint(0,6) if tables==0 else rng.randint(0,1) if tables==1 else 0
                    coverage['rows'].add(row_count); tables+=1
                    for row in range(row_count):
                        cells=[f'{seed}_{part}_{student}_{row}_{c} '+['2026-11-01','A·B','3/5','X｜Y','42'][(c+seed)%5]+f' / tail{c}' for c in range(n)]
                        text='| '+' | '.join(cells)+' |'; rows.append((len(lines),text,cells));lines.append(text)
            for name,word in [('미확인 사항','제출 필요'),('다음 확인','Check book')]:
                lines.append(name)
                statements_count=rng.randint(0,3); coverage['statements'].add(statements_count)
                for j in range(statements_count):
                    text=word+' '+str(j);statements.append((len(lines),text,word));lines.append(text)
            footer='읽은 원천 Fixture '+str(seed);closing='이번 결과는 읽기만 했고 저장한 것은 없습니다.'
            if rng.choice([True,False]):lines.append(footer)
            if rng.choice([True,False]):lines.append(closing)
            source='\n'.join(lines)
            check(seed,'identity',source,source,'pass')
            if rows:
                index,text,cells=rng.choice(rows)
                check(seed,'drop_row',source,'\n'.join(lines[:index]+lines[index+1:]),'FAIL')
                changed=text.replace(cells[0],cells[0]+' changed',1)
                check(seed,'change_field',source,'\n'.join(lines[:index]+[changed]+lines[index+1:]),'FAIL')
                check(seed,'duplicate_row',source,'\n'.join(lines[:index]+[text]+lines[index:]),'FAIL')
                moved=lines[:index]+lines[index+1:]
                target=moved.index('미확인 사항')+1
                moved.insert(target,text)
                check(seed,'move_context',source,'\n'.join(moved),'FAIL')
                current=''
                for line in lines[:index]:
                    if line.startswith('# Part '): current=''
                    elif line.startswith('## Student '): current=line
                alternatives=[i+1 for i,line in enumerate(lines) if line.startswith('## Student ') and line!=current]
                if alternatives:
                    target=rng.choice(alternatives); relocated=lines[:index]+lines[index+1:]
                    if target>index: target-=1
                    relocated.insert(target,text)
                    check(seed,'move_student',source,'\n'.join(relocated),'FAIL')
                for column in [seed % len(cells)]:
                    wrapped=text.replace(cells[column],cells[column].replace(' / ','\n'),1)
                    answer='\n'.join(lines[:index]+[wrapped]+lines[index+1:])
                    check(seed,'cell_break_'+str(column),source,answer,'pass')
                    check(seed,'cell_break_reverse_'+str(column),answer,source,'pass')
                # Swap two adjacent rows only when no heading/header intervenes.
                pairs=[(a,b) for a,b in zip(rows,rows[1:]) if b[0]==a[0]+1]
                if pairs:
                    a,b=rng.choice(pairs); swapped=list(lines);swapped[a[0]],swapped[b[0]]=swapped[b[0]],swapped[a[0]]
                    check(seed,'reorder_rows',source,'\n'.join(swapped),'pass')
            if statements:
                index,text,word=rng.choice(statements)
                check(seed,'drop_statement',source,'\n'.join(lines[:index]+lines[index+1:]),'FAIL')
                opposite=text.replace(word,OPPOSITES[word],1)
                check(seed,'opposite_statement',source,'\n'.join(lines[:index]+[opposite]+lines[index+1:]),'FAIL')
            check(seed,'invented_row',source,source+'\n| New | Invented |','FAIL')
            position=rng.randrange(len(lines)+1)
            comment=rng.choice(['One stateless comment','# New comment','Invented 학원 내 처리'])
            check(seed,'one_comment',source,'\n'.join(lines[:position]+[comment]+lines[position:]),'pass')
            check(seed,'two_comments',source,'\n'.join(lines[:position]+['# First comment','Other 학원 내 처리']+lines[position:]),'FAIL')
            check(seed,'state_comment',source,'\n'.join(lines[:position]+['# 서버 확인']+lines[position:]),'FAIL')
            check(seed,'full_width_delimiters',source,source.replace(' | ',' ｜ '),'pass')
            if footer in lines:check(seed,'drop_footer',source,'\n'.join(x for x in lines if x!=footer),'pass')
            if closing in lines:check(seed,'drop_closing',source,'\n'.join(x for x in lines if x!=closing),'pass')
        elapsed=time.perf_counter()-start
        self.assertLess(elapsed,5.0,('property_seconds',elapsed))
        self.assertEqual(counts['identity'],CASES)
        for key,values in {'parts':range(1,4),'students':range(4),'columns':range(2,6),'rows':range(7),'statements':range(4)}.items():
            self.assertEqual(coverage[key],set(values),key)
        for label in ['drop_row','change_field','move_context','move_student','duplicate_row','drop_statement','opposite_statement','invented_row','two_comments','state_comment','reorder_rows','full_width_delimiters','one_comment','drop_footer','drop_closing']:
            self.assertGreater(counts.get(label,0),0,label)
        for column in range(5):self.assertGreater(counts.get('cell_break_'+str(column),0),0)
        print(f'property_cases={CASES} assertions={checked} seconds={elapsed:.3f}')
