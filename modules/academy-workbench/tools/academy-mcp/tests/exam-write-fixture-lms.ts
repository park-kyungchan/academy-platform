import {lmsRequestRole,lmsRoute,type LmsRequestRole} from './network-deny.ts';
import {examPageSha256,type ExpectedExamWireHashes} from '../../../../../apps/academy/dc-gang-a/src/lms/examPaperWrite';
import {paperExamListForm,parsePaperExamList} from '../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead';
import type {ExamPaperBackend} from '../src/exam-write-tools.ts';

// A recorded synthetic LMS for tests/exam-paper-writes.test.ts: the fa and da creation and assignment pages whose functions are pinned
// (invented function bodies, hashed through the test seam `expectedWireHashes`), and the servlets the exam pair talks to, with the
// state a paper goes through (a number allocated by the Auto page, the items staged, the paper saved, the paper assigned). Invented numeric keys
// and names only; no private page, no live request. Each request is classed by the same table as tests/network-deny.ts, and a request classed
// write is answered `write_forbidden` (403) while `allowWrite` is false.
export const NAME_A='Invented Alpha',NAME_B='Invented Beta',KEY_A='77001',KEY_B='77002',GRADE='2',CLASS_A='31',CLASS_B='32',CLS_NAME_A='Invented Class A',CLS_NAME_B='Invented Class B';
export const FIRST_TESTNO=424001,SOURCE_PAPER='6001',COURSE_PAPER='6100',COURSE_NO=3301;
export const COOKIE_CANARY='invented-cookie-canary-4471';
const ORIGIN='https://dc.gang-a.kr';
export const itemKeys=(n:number,base=100000001)=>Array.from({length:n},(_,i)=>String(base+i));

const functions=(names:string[])=>names.map((n,i)=>`function ${n}(a) {\n  var v = ${i + 1};\n  return v + 1;\n}`).join('\n');
// The live Auto page of 2026-11-28 (finding ama-live-exam-form-members): `userForm` first (its own members, never posted), the input `lecture_key` between the two
// forms, then `form1` (multipart) with its members in the live page order; the da form1 adds `courseNo` after `testgubun` and `useterm` after `grade`.
const LIVE_FORM1=['p_process','p_original_testno','p_prepare_testno','p_bookno','p_evalgubunno','p_testgubunno','p_gradeno','p_subjectno','p_total_cnt','p_o_cnt','p_s_cnt','p_d_cnt',
  'paper_type','evalgubun','exam_test','testgubun','testname','private_chk','total','object','objectviews','subjectquestion','descriptquestion','grade','bunit1','munit1','bunit2','munit2',
  'title_option','keyword_title','makecollege','maker','makedate','subject','book'];
export const USER_FORM_MEMBERS=['p_process','p_prino','p_groupno','p_url','fran_no','pri_no','mem_type'];
// `unsubmitted` gives form1 the two controls of the live page of 2026-11-28T18:09Z that a browser does not submit: `private_chk` an unchecked checkbox and
// `subject` a select without an option.
export const creationPage=(family:'fa'|'da',testno:number|string,body='',members:{omit?:string[];unsubmitted?:boolean}={}):string=>{
  const omit=members.omit??[];
  const select=(n:string,options:string)=>`<select name="${n}">${options}</select>`;
  const field=(n:string):string=>{
    if(omit.includes(n))return '';
    if(members.unsubmitted&&n==='private_chk')return '<input type="checkbox" name="private_chk" value="1">';
    if(members.unsubmitted&&n==='subject')return select(n,'');
    if(n==='testgubun')return select(n,'<option value="-1">-</option><option value="1180">a</option><option value="1161">b</option><option value="1320">c</option>');
    if(n==='makecollege')return select(n,'<option value="-1">-</option><option value="">none</option><option value="501">Invented college</option>');
    if(['grade','bunit1','munit1','bunit2','munit2','subject','book'].includes(n))return select(n,`<option value="x-${n}" selected>x</option>`);
    const value=n==='p_original_testno'?String(testno):n==='evalgubun'?(family==='fa'?'1001':'1000'):n==='private_chk'?'0':n==='objectviews'?'invented-views':n==='makedate'?'2040-01-16':n==='title_option'?'old':'x-'+n;
    return `<input type="${['testname','keyword_title','maker','makedate'].includes(n)?'text':'hidden'}" name="${n}" value="${value}">`;
  };
  const inputs=LIVE_FORM1.flatMap(n=>{
    const out=[field(n)];
    if(family==='da'&&n==='testgubun')out.push(omit.includes('courseNo')?'':select('courseNo',`<option value="-1">-</option><option value="${COURSE_NO}">Invented course</option>`));
    if(family==='da'&&n==='grade')out.push('<input type="hidden" name="useterm" value="2">');
    return out;
  }).filter(Boolean).join('\r\n');
  const userForm=`<form name="userForm" method="post">\r\n${USER_FORM_MEMBERS.map(n=>`<input type="hidden" name="${n}" value="${n==='p_process'?'Auto':n==='p_url'?'a&amp;b=1':'u-'+n}">`).join('\r\n')}\r\n</form>\r\n`;
  return `<html><body>\r\n<script>\r\n${functions(['validationTestInfo','requestSave','onSaveButtonClick'])}\r\n${body}</script>\r\n${userForm}<input type="hidden" name="lecture_key" value="u-lecture_key">\r\n`
    +`<form name="form1" method="post" enctype="multipart/form-data">\r\n${inputs}\r\n</form></body></html>`;
};
export const assignmentPage=(body='')=>`<html><body>\r\n<script>\r\n${functions(['validate','onSave','showSaveResult'])}\r\n${body}</script>\r\n</body></html>`;
export const pageHashes=():Required<{[F in 'fa'|'da']:{creation:string;assignment:string}}>=>({
  fa:{creation:examPageSha256('creation',creationPage('fa',1))!,assignment:examPageSha256('assignment',assignmentPage())!},
  da:{creation:examPageSha256('creation',creationPage('da',1))!,assignment:examPageSha256('assignment',assignmentPage())!}}) satisfies ExpectedExamWireHashes;

export interface FixtureRequest{method:string;route:string;role:LmsRequestRole;path:string;query:URLSearchParams;form:URLSearchParams;multipart:[string,string][]|null}
type Answer=string|Response;
export function fixtureExamLms(){
  const requests:FixtureRequest[]=[];
  const students=[
    {student_name:NAME_A,mem_id:'invented-login-a',cls_name:CLS_NAME_A,grade_no:Number(GRADE),cls_no:Number(CLASS_A),pri_no:Number(KEY_A)},
    {student_name:NAME_B,mem_id:'invented-login-b',cls_name:CLS_NAME_B,grade_no:Number(GRADE),cls_no:Number(CLASS_B),pri_no:Number(KEY_B)},
  ];
  const state={
    allowWrite:true,
    students,
    nextTestno:FIRST_TESTNO,
    pages:{fa:{creation:null as null|((testno:number)=>string),assignment:assignmentPage()},da:{creation:null as null|((testno:number)=>string),assignment:assignmentPage()}},
    sourcePapers:new Map<string,{sel_type:string;outer_lecture_key:number|null;lecture_key:number}[]>([
      [SOURCE_PAPER,itemKeys(4).map((k,i)=>({sel_type:i<2?'obj':i<3?'sbj':'des',outer_lecture_key:Number(k),lecture_key:5000+i}))],
      [COURSE_PAPER,itemKeys(30,200000001).map((k,i)=>({sel_type:i<20?'obj':i<26?'sbj':'des',outer_lecture_key:Number(k),lecture_key:6000+i}))],
    ]),
    coursePaperAnswer:null as null|(()=>Answer),
    studentAnswer:null as null|(()=>Answer),
    saveMunhangAnswer:null as null|(()=>Answer),
    checkCount:1,
    paperSaveAnswer:null as null|(()=>Answer),
    paperSaveSaves:true,
    hangSaveAnswer:null as null|(()=>Answer),
    hangSaveAssigns:true,
    validateAnswer:null as null|(()=>Answer),
    autoModifyAnswer:null as null|(()=>Answer),
    allocated:[] as number[],
    staged:new Map<string,{sel_type:string;outer_lecture_key:number;lecture_key:number}[]>(),
    saved:new Map<string,{testname:string}>(),
    assigned:new Map<string,string[]>(),
    onRequest:null as null|((request:FixtureRequest)=>void|Promise<void>),
  };
  const xml=(body:string)=>new Response('<?xml version="1.0" encoding="UTF-8"?><root>'+body+'</root>');
  const transport=async(url:string|URL,init?:RequestInit):Promise<Response>=>{
    const u=new URL(String(url));
    const rawBody=init?.body,formText=typeof rawBody==='string'?rawBody:'',form=new URLSearchParams(formText);
    const {path,operation}=lmsRoute(url,typeof rawBody==='string'?rawBody:undefined);
    const op=operation||form.get('reqCmd')||'';
    const role=lmsRequestRole(url,typeof rawBody==='string'?rawBody:undefined,init?.method);
    const multipart=rawBody instanceof FormData?[...rawBody.entries()].map(([k,v])=>[k,String(v)] as [string,string]):null;
    const request:FixtureRequest={method:(init?.method??'GET').toUpperCase(),route:op?path+' '+op:path,role,path,query:u.searchParams,form,multipart};
    requests.push(request);
    await state.onRequest?.(request);
    if(role==='write'&&!state.allowWrite)return new Response('write_forbidden',{status:403});
    const answer=(out:Answer)=>typeof out==='string'?new Response(out):out;
    const family=path.includes('.tutor.fa.')?'fa':path.includes('.tutor.da.')?'da':null;
    const q=u.searchParams;
    if(path.endsWith('.TestPageRegServlet')&&family){
      if(request.method==='GET'&&q.get('p_process')==='Auto'){
        const testno=state.nextTestno++;state.allocated.push(testno);
        return new Response(state.pages[family].creation?state.pages[family].creation!(testno):creationPage(family,testno));
      }
      if(request.method==='GET'&&q.get('reqCmd')==='CheckMunhang')return xml(`<count>${state.checkCount}</count><qno>7</qno>`);
      if(request.method==='POST'&&q.get('reqCmd')==='Save'){
        if(state.paperSaveAnswer)return answer(state.paperSaveAnswer());
        const get=(n:string)=>multipart?.find(([k])=>k===n)?.[1]??'';
        if(state.paperSaveSaves)state.saved.set(get('p_original_testno'),{testname:get('testname')});
        return new Response(`<html><script>var logic = "${state.paperSaveSaves?'UPLOAD_OK':'UPLOAD_FAIL'}";</script></html>`);
      }
      if(request.method==='GET'&&q.get('p_process')==='AutoModify'){
        if(state.autoModifyAnswer)return answer(state.autoModifyAnswer());
        const saved=state.saved.get(q.get('p_original_testno')??'');
        return new Response(saved?`<html><form name="form1"><input name="p_original_testno" value="${q.get('p_original_testno')}"><input name="testname" value="${saved.testname}"></form></html>`:`<html><form name="form1"><input name="p_original_testno" value="${q.get('p_original_testno')}"><input name="testname" value=""></form></html>`);
      }
    }
    if(path.endsWith('.TestHangServlet')&&family){
      if(request.method==='GET'&&q.get('p_process')==='Main')return new Response(state.pages[family].assignment);
      const names=(): string[]=>[...(q.get('objecter')??'').split('-').filter(Boolean)].map(e=>students.find(s=>String(s.pri_no)===e.split('!')[1])?.student_name??'?');
      if(request.method==='GET'&&q.get('reqCmd')==='Validate'){
        if(state.validateAnswer)return answer(state.validateAnswer());
        const held=state.assigned.get(q.get('pno')??'')??[];
        return xml(held.length?`<result>${held.join(', ')}</result><info>Invented earlier test</info>`:'<result>success</result>');
      }
      if(request.method==='GET'&&q.get('reqCmd')==='Save'){
        if(state.hangSaveAnswer)return answer(state.hangSaveAnswer());
        if(state.hangSaveAssigns)state.assigned.set(q.get('pno')!,[...(state.assigned.get(q.get('pno')!)??[]),...names()]);
        return xml('<result>success</result>');
      }
    }
    if(path.endsWith('common.TestpageSelectServlet')&&request.method==='GET'&&q.get('reqCmd')==='SaveMunhang'){
      if(state.saveMunhangAnswer)return answer(state.saveMunhangAnswer());
      const rows=(q.get('exam_list')??'').split('|,|').map((e,i)=>{const [k,t]=e.split('|_|');return {sel_type:t==='1'?'obj':t==='2'?'sbj':'des',outer_lecture_key:Number(k),lecture_key:9000+i};});
      state.staged.set(q.get('p_original_testno')!,rows);
      const count=(t:string)=>rows.filter(r=>r.sel_type===t).length;
      return xml(`<result>result_ok</result><start_bunit_no>11</start_bunit_no><start_munit_no>12</start_munit_no><end_bunit_no>13</end_bunit_no><end_munit_no>14</end_munit_no>`
        +`<total_cnt>${rows.length}</total_cnt><total_object>${count('obj')}</total_object><total_subject>${count('sbj')}</total_subject><total_descript>${count('des')}</total_descript>`
        +`<book_no>21</book_no><subject_no>22</subject_no><grade_no>23</grade_no>`);
    }
    if(path.endsWith('common.TestpageSelectExServlet')&&request.method==='POST'&&form.get('p_process')==='getTestExamList'){
      const paper=form.get('p_no')!;
      const rows=state.staged.get(paper)??state.sourcePapers.get(paper)??[];
      return new Response(JSON.stringify({result:'OK',exam_list:rows.map(r=>({...r,examtext_key:0}))}));
    }
    if(path.endsWith('common.ObjectSelectServlet')&&request.method==='POST'&&form.get('reqCmd')==='getStudentList'){
      if(state.studentAnswer)return answer(state.studentAnswer());
      const want=form.get('studentName')??'';
      return new Response(JSON.stringify({result:'OK',student_list:state.students.filter(s=>s.student_name.includes(want))}));
    }
    if(path.endsWith('tutor.diag.DiagnostManageServlet')&&request.method==='POST'&&form.get('reqCmd')==='GetPaperNo'){
      if(state.coursePaperAnswer)return answer(state.coursePaperAnswer());
      return new Response(JSON.stringify({pNo:Number(COURSE_PAPER),result:'OK'}));
    }
    throw Error('unexpected_fixture_request:'+request.method+' '+request.route);
  };
  return {requests,state,transport,
    writes:()=>requests.filter(r=>r.role==='write'),reads:()=>requests.filter(r=>r.role==='read'),
    routes:()=>requests.map(r=>r.method+' '+r.route+(r.role==='write'?' [write]':''))};
}
export type FixtureExamLms=ReturnType<typeof fixtureExamLms>;

const text=async(response:Response)=>{if(!response.ok)throw Error('academy_transport_failed');return response.text();};
const FORM={'Content-Type':'application/x-www-form-urlencoded; charset=UTF-8','X-Requested-With':'XMLHttpRequest'};
/** The port of the service over the recorded LMS: what AcademyHttpClient's methods send, without the login exchange. */
export const examFixturePort=(lms:FixtureExamLms):ExamPaperBackend=>({
  readExamRoute:async path=>text(await lms.transport(ORIGIN+path,{method:'GET'})),
  postExamRead:async (path,fields)=>text(await lms.transport(ORIGIN+path,{method:'POST',headers:FORM,body:new URLSearchParams(fields).toString()})),
  readPaperExamList:async paperNo=>parsePaperExamList(await text(await lms.transport(ORIGIN+'/servlet/controller.common.TestpageSelectExServlet',
    {method:'POST',headers:FORM,body:new URLSearchParams(paperExamListForm(paperNo)).toString()}))),
  writeExamGet:async path=>text(await lms.transport(ORIGIN+path,{method:'GET'})),
  writeExamMultipart:async (path,members)=>{const body=new FormData();for(const [n,v] of members)body.append(n,v);return text(await lms.transport(ORIGIN+path,{method:'POST',body}));},
});
