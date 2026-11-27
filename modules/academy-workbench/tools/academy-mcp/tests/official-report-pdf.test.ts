import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {lstat,mkdir,mkdtemp,realpath,rm,symlink,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {OfficialReportPdfService,officialReportPdfGetSchema,type CourseBookReportBackend} from '../src/official-report-pdf.ts';
import type {StudyqRunner} from '../src/studyq-runner.ts';
import {projectCourseBookReport} from '../../../../../apps/academy/dc-gang-a/src/lms/courseBookReport.ts';

// academy_official_report_pdf_get (src/official-report-pdf.ts) with an injected backend and runner:
// the output root and path rules, the answer and the refusals. Invented values only.
const STUDENT='Invented Student',KEY='770011',COURSE='560013',BOOK='430019';
const report=projectCourseBookReport({block1:[{'1':'가우스 초등 1-1 Invented','2':'2040-01-01','3':'2040-03-01','4':STUDENT}],block2:[{'1':5,'3':82.5,'4':75,'5':'c'}],
  block5:Array.from({length:9},(_,i)=>({'1':i+1,'2':'t'+i,'3':10,'4':70,'5':80,'6':'03-01'})),
  bunit_list:[{'1':'Invented chapter','3':80,'4':70,exam_result:[{'1':1,'2':'Invented type','3':2,'4':1,'5':66,'6':'O'}]}],
  block9:[{'2':1,'3':2,'4':3,'5':4}],block10:[{'1':90},{'1':80},{'1':70},{'1':60}],block11:[{'2':71}],block12:[{'2':'u','3':3,'4':4,'5':70}]},
  {studentName:STUDENT,studentKey:KEY,courseKey:COURSE,bookNo:BOOK});
let base:string;
beforeAll(async()=>{base=await realpath(await mkdtemp(resolve(tmpdir(),'official-report-')));});
afterAll(async()=>{await rm(base,{recursive:true,force:true});});
const now=()=>new Date('2040-01-16T09:00:00.123Z');
function fixture(name:string,over:Partial<{answer:Awaited<ReturnType<CourseBookReportBackend['readCourseBookReportByName']>>;rendered:Awaited<ReturnType<StudyqRunner['renderCourseBook']>>}>={}){
  const calls={backend:[] as unknown[],runner:[] as {input:unknown;path:string}[]};
  const backend:CourseBookReportBackend={async readCourseBookReportByName(input){calls.backend.push(input);
    return over.answer??{kind:'report',student:{studentId:KEY,displayName:STUDENT},courseKey:COURSE,bookNo:BOOK,bindingSource:'historical_key_registry',report};}};
  const runner:StudyqRunner={async renderCourseBook(input,path){calls.runner.push({input,path});
    return over.rendered??{status:'rendered',pages:8,sha256:'a'.repeat(64),outputPath:path,parityOk:true,parityMismatches:0,parityRawRn:0};}};
  const root=resolve(base,name,'official-reports');
  return {calls,root,service:new OfficialReportPdfService(backend,runner,root,now)};
}

describe('academy_official_report_pdf_get',()=>{
  test('official_report_renders_course_book_under_the_owner_root',async()=>{
    const f=fixture('render');
    const answer=await f.service.get({reportType:'course_book',displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY}) as any;
    const path=resolve(f.root,'course_book-'+BOOK+'-20400116T090000Z.pdf');
    expect(answer).toMatchObject({status:'rendered',reportType:'course_book',outputPath:path,pages:8,parityOk:true,parityMismatches:0,parityRawRn:0,
      courseKey:COURSE,bookNo:BOOK,bindingSource:'historical_key_registry',academyWrites:0,
      reportSummary:{bookTitle:'가우스 초등 1-1 Invented',period:'2040-01-01 ~ 2040-03-01',myAverage:82.5,allAverage:75,chapterCount:1},
      truncation:{testsDropped:1,chaptersDropped:0,questionsDropped:0,munitsDropped:0}});
    expect(f.calls.backend).toEqual([{displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY}]);
    expect(f.calls.runner).toEqual([{input:report.studyqInput,path}]);
    expect(path).not.toContain('Invented');
    const stat=await lstat(f.root);expect(stat.isDirectory()&&(stat.mode&0o777)===0o700).toBe(true);
    // studentName and params.bookLabel name the student and the book.
    await f.service.get({reportType:'course_book',studentName:STUDENT,params:{bookLabel:'가우스 1-1'},outputPath:'mine.pdf'});
    expect(f.calls.backend.at(-1)).toEqual({displayName:STUDENT,bookLabel:'가우스 1-1'});
    expect(f.calls.runner.at(-1)!.path).toBe(resolve(f.root,'mine.pdf'));
  });

  test('official_report_output_path_confined_and_never_overwritten',async()=>{
    const f=fixture('paths');
    await mkdir(f.root,{recursive:true,mode:0o700});
    await writeFile(resolve(f.root,'taken.pdf'),'x');
    await symlink(base,resolve(f.root,'escape'));
    for(const [outputPath,code] of [['report.txt','output_path_invalid'],['../report.pdf','output_path_invalid'],['sub/../report.pdf','output_path_invalid'],
      [resolve(base,'outside.pdf'),'output_path_invalid'],['escape/report.pdf','output_path_invalid'],['missing/report.pdf','output_path_invalid'],['taken.pdf','output_exists']] as const){
      expect({outputPath,answer:await f.service.get({reportType:'course_book',displayName:STUDENT,outputPath})}).toMatchObject({outputPath,answer:{status:'unavailable',reasonCode:code}});
    }
    // Refused before any LMS request.
    expect(f.calls.backend).toEqual([]);expect(f.calls.runner).toEqual([]);
    // An output root that is a symbolic link is refused.
    const target=resolve(base,'paths-target');await mkdir(target);
    const linked=new OfficialReportPdfService(fixture('x').service.backend,fixture('x').service.runner,resolve(base,'paths-link'),now);
    await symlink(target,resolve(base,'paths-link'));
    expect(await linked.get({reportType:'course_book',displayName:STUDENT})).toMatchObject({status:'unavailable',reasonCode:'output_root_unqualified'});
  });

  test('official_report_refusals_and_interview',async()=>{
    const refused=fixture('refused',{rendered:{status:'refused',code:'input_invalid',detail:'range:bunit_list[0].questions[0].type'}});
    expect(await refused.service.get({reportType:'course_book',displayName:STUDENT})).toMatchObject({status:'refused',reasonCode:'studyq_input_invalid',studyqDetail:'range:bunit_list[0].questions[0].type'});
    const interview=fixture('interview',{answer:{kind:'interview_required',student:{studentId:KEY,displayName:STUDENT},bindings:[{bookLabel:'A',bookNo:'1'},{bookLabel:'B',bookNo:'2'}],
      reason:'multiple_book_bindings_select_one',observedAt:'2040-01-16T09:00:00.000Z'}});
    expect(await interview.service.get({reportType:'course_book',displayName:STUDENT})).toMatchObject({status:'interview_required',bindings:[{bookLabel:'A'},{bookLabel:'B'}]});
    expect(interview.calls.runner).toEqual([]);
    const conflict=fixture('conflict',{answer:{kind:'report',student:{studentId:KEY,displayName:STUDENT},courseKey:COURSE,bookNo:'430099',report}});
    expect(await conflict.service.get({reportType:'course_book',displayName:STUDENT,courseKey:COURSE,bookNo:BOOK,studentKey:KEY})).toMatchObject({status:'unavailable',reasonCode:'official_report_scope_conflict'});
    expect(await new OfficialReportPdfService(null,null,resolve(base,'none')).get({reportType:'course_book',displayName:STUDENT}))
      .toMatchObject({status:'unavailable',reasonCode:'official_report_backend_not_configured',academyWrites:0});
    expect(await new OfficialReportPdfService(fixture('y').service.backend,null,resolve(base,'none')).get({reportType:'course_book',displayName:STUDENT}))
      .toMatchObject({status:'unavailable',reasonCode:'studyq_runner_not_configured'});
    // Input refused before anything runs.
    for(const bad of [{reportType:'cism',displayName:STUDENT},{reportType:'course_book'},{reportType:'course_book',displayName:STUDENT,studentName:'Other Invented'},
      {reportType:'course_book',displayName:STUDENT,courseKey:COURSE},{reportType:'course_book',displayName:STUDENT,params:{unknown:true}},
      {reportType:'course_book',displayName:STUDENT,courseKey:'0',bookNo:BOOK,studentKey:KEY}])
      expect({bad,ok:officialReportPdfGetSchema.safeParse(bad).success}).toEqual({bad,ok:false});
  });
});
