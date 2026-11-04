import {expect,test} from 'bun:test';
import {deriveTextbookSectionTitlePattern,isTextbookSectionPaperTitle,mergeTextbookSectionRows} from '../../src/lms/textbookSectionIndex';
import {createAcademyReadPlan,executeAcademyRead} from '../../src/lms/academyReadProtocol';

const studentName='Invented Learner';
const wrongTitle='[필수예제] [가우스] 5-3 진분수의 뺄셈 - Invented Learner';
const perfectTitle='[필수예제] [가우스] 5-4 대분수의 뺄셈 - Invented Learner';
const wrongRow={sourceTitle:wrongTitle,paperKey:'81003',attemptKey:'82003',sourceDateLiteral:'11-03'};
const paperRows=[
  {testingName:wrongTitle,paperKey:'81003',attemptKey:'82003',applyDateLiteral:'2026-11-03 10:00:00'},
  {testingName:perfectTitle,paperKey:'81004',attemptKey:'82004',applyDateLiteral:'2026-11-03 10:05:00'},
  {testingName:'[필수예제] [다른책] 5-4 대분수의 뺄셈 - Invented Learner',paperKey:'81005',attemptKey:'82005',applyDateLiteral:'2026-11-03 10:06:00'},
  {testingName:'[유형다지기] [가우스] 5-4 대분수의 뺄셈 - Invented Learner',paperKey:'81006',attemptKey:'82006',applyDateLiteral:'2026-11-03 10:07:00'},
  {testingName:perfectTitle,paperKey:'81007',attemptKey:'82007',applyDateLiteral:null},
];

test('title pattern anchors on the book bracket token and student suffix from wrong-note titles',()=>{
  const pattern=deriveTextbookSectionTitlePattern([wrongTitle],studentName);
  expect(pattern).toMatchObject({pairs:[{activity:'필수예제',book:'가우스'}],studentSuffix:' - Invented Learner'});
  expect(isTextbookSectionPaperTitle(perfectTitle,pattern!)).toBe(true);
  expect(isTextbookSectionPaperTitle('[필수예제] [다른책] 5-4 대분수의 뺄셈 - Invented Learner',pattern!)).toBe(false);
  expect(isTextbookSectionPaperTitle('[유형다지기] [가우스] 5-4 대분수의 뺄셈 - Invented Learner',pattern!)).toBe(false);
  expect(isTextbookSectionPaperTitle('[필수예제] [가우스] 5-4 대분수의 뺄셈 - Other Learner',pattern!)).toBe(false);
  expect(deriveTextbookSectionTitlePattern([],studentName)).toBeNull();
  expect(deriveTextbookSectionTitlePattern(['No brackets here'],studentName)).toBeNull();
});

test('an activity/book pair never observed together does not join, even when both tokens appeared separately',()=>{
  const pattern=deriveTextbookSectionTitlePattern(
    ['[필수예제] [BookA] 1-1 x - Invented Learner','[유형다지기] [BookB] 1-1 x - Invented Learner'],studentName);
  expect(isTextbookSectionPaperTitle('[필수예제] [BookB] 9-9 z - Invented Learner',pattern!)).toBe(false);
  expect(isTextbookSectionPaperTitle('[유형다지기] [BookB] 9-9 z - Invented Learner',pattern!)).toBe(true);
  const merged=mergeTextbookSectionRows(
    [{sourceTitle:'[필수예제] [BookA] 1-1 x - Invented Learner',paperKey:'1',attemptKey:'1',sourceDateLiteral:null}],
    [{testingName:'[필수예제] [BookB] 9-9 z - Invented Learner',paperKey:'2',attemptKey:'2',applyDateLiteral:null}],pattern);
  expect(merged).toHaveLength(1);
});

test('merge keeps wrong-note rows, adds all-correct paper rows, and refuses attempt/title conflicts',()=>{
  const pattern=deriveTextbookSectionTitlePattern([wrongTitle],studentName);
  const merged=mergeTextbookSectionRows([wrongRow],paperRows.slice(0,4),pattern);
  expect(merged.map(r=>r.sourceTitle)).toEqual([wrongTitle,perfectTitle]);
  expect(merged[0]).toMatchObject({attemptKey:'82003',sources:['wrong_answer_note','student_paper_search']});
  expect(merged[1]).toMatchObject({attemptKey:'82004',paperKey:'81004',sources:['student_paper_search'],sourceDateLiteral:'2026-11-03 10:05:00'});
  // Same title on a second attempt stays a separate row: ambiguity is downstream, never guessed away.
  expect(mergeTextbookSectionRows([wrongRow],paperRows,pattern).filter(r=>r.sourceTitle===perfectTitle)).toHaveLength(2);
  expect(()=>mergeTextbookSectionRows([wrongRow],[{testingName:perfectTitle,paperKey:'81003',attemptKey:'82003',applyDateLiteral:null}],pattern))
    .toThrow('textbook_section_title_conflict_for_attempt');
  // Without a derivable pattern, paper rows never join a book.
  expect(mergeTextbookSectionRows([wrongRow],paperRows,null)).toHaveLength(1);
});

const progress='<button onclick="createBookIncorrectPapers(693,2517,77001)">Gauss fraction book</button>';
const notes={result:'ok',noteResultList:[{pri_no:77001,name:studentName,p_no:81003,testing_no:82003,testing_name:wrongTitle,apply_date:'11-03'}]};
const details:Record<string,unknown>={'82003':{result:'OK',print_list:[{pri_no:77001,student_name:studentName,sub_title:wrongTitle,score:84,
    exam_list:[{exam_no:1,lecture_key:101,scoring_result:'O'},{exam_no:2,lecture_key:102,scoring_result:'X'}]}]},
  '82004':{result:'OK',print_list:[{pri_no:77001,student_name:studentName,sub_title:perfectTitle,score:100,
    exam_list:[{exam_no:1,lecture_key:201,scoring_result:'O'},{exam_no:2,lecture_key:202,scoring_result:'O'}]}]}};
function fetcher(url:string,init:RequestInit){
  const body=new URLSearchParams(init.body as string);
  if(url.includes('getTextBookProgress'))return Promise.resolve(new Response(progress));
  if(body.get('p_process')==='getWrongAnswerNoteResult')return Promise.resolve(Response.json(notes));
  const condition=JSON.parse(body.get('condition')!);
  const detail=details[String(condition.testing_no)];
  if(!detail)throw Error('unexpected_testing_no');
  return Promise.resolve(Response.json(detail));
}
const request={family:'textbook',teacherLabel:'Invented Teacher',studentKey:'77001',studentName,courseKey:'693',bookNo:'2517',gradeViewAdmission:true};

test('grading finds the 100-point section through paper search and still grades the wrong-note section',async()=>{
  const plan=createAcademyReadPlan({...request,sourceTitles:[perfectTitle,wrongTitle]});
  const result=await executeAcademyRead(plan,{origin:'https://dc.gang-a.kr',authenticatedTeacherVisible:true,fetcher,
    textbookPaperSearch:async()=>({status:'observed',rows:paperRows.slice(0,4)})});
  const perfect=result.records.find(r=>r.sourceTitle===perfectTitle)!;
  expect(perfect).toMatchObject({score:100,attemptKey:'82004',paperKey:'81004'});
  expect(perfect.items.map(i=>i.scoring)).toEqual(['O','O']);
  expect(result.records.find(r=>r.sourceTitle===wrongTitle)).toMatchObject({score:84,attemptKey:'82003'});
  expect(result.unknownStates).toContain('student_paper_search_one_observed_page_not_complete_history');
});

test('a title absent from both sources is distinct, and unavailable paper search never licenses that claim',async()=>{
  const missing='[필수예제] [가우스] 9-9 없는 단원 - Invented Learner';
  const plan=createAcademyReadPlan({...request,sourceTitles:[missing]});
  await expect(executeAcademyRead(plan,{origin:'https://dc.gang-a.kr',authenticatedTeacherVisible:true,fetcher,
    textbookPaperSearch:async()=>({status:'observed',rows:paperRows.slice(0,4)})})).rejects.toThrow('textbook_section_not_in_either_observed_source');
  await expect(executeAcademyRead(plan,{origin:'https://dc.gang-a.kr',authenticatedTeacherVisible:true,fetcher,
    textbookPaperSearch:async()=>({status:'unavailable',rows:[]})})).rejects.toThrow('source_selection_missing_or_ambiguous');
  // Without any paper-search provider the wrong-note section still grades.
  const wrongOnly=createAcademyReadPlan({...request,sourceTitles:[wrongTitle]});
  const result=await executeAcademyRead(wrongOnly,{origin:'https://dc.gang-a.kr',authenticatedTeacherVisible:true,fetcher});
  expect(result.records[0]).toMatchObject({score:84});
  expect(result.unknownStates).toContain('student_paper_search_unavailable_wrong_note_index_only');
});

test('without a derivable title pattern, absence is selection ambiguity, never a false not-in-either claim',async()=>{
  const plainTitle='Invented existing section';
  const plainNotes={result:'ok',noteResultList:[{pri_no:77001,name:studentName,p_no:81003,testing_no:82003,testing_name:plainTitle,apply_date:'11-03'}]};
  const plainFetcher=(url:string,init:RequestInit)=>{
    const body=new URLSearchParams(init.body as string);
    if(url.includes('getTextBookProgress'))return Promise.resolve(new Response(progress));
    if(body.get('p_process')==='getWrongAnswerNoteResult')return Promise.resolve(Response.json(plainNotes));
    throw Error('detail must not be read for an unanchored title');
  };
  const missing='Missing exact title';
  const plan=createAcademyReadPlan({...request,sourceTitles:[missing]});
  await expect(executeAcademyRead(plan,{origin:'https://dc.gang-a.kr',authenticatedTeacherVisible:true,fetcher:plainFetcher,
    textbookPaperSearch:async()=>({status:'observed',rows:[{testingName:missing,paperKey:'81004',attemptKey:'82004',applyDateLiteral:null}]})}))
    .rejects.toThrow('source_selection_missing_or_ambiguous');
});
