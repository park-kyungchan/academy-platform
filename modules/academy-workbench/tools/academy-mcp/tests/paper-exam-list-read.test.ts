import {describe,expect,test} from 'bun:test';
import {lmsRequestRole,writeForbiddenTransport} from './network-deny.ts';
import {AcademyHttpClient} from '../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts';
import {paperExamListForm,parsePaperExamList} from '../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead.ts';

// The exam-list read that the gated exam write keeps using (src/exam-write-tools.ts through the native backend):
// paperExamListForm, parsePaperExamList and AcademyHttpClient.readPaperExamList of
// apps/academy/dc-gang-a/src/lms/paperPrintRead.ts over a fixture transport. These tests were moved here unchanged
// from the retired tests/paper-print-pdf.test.ts. Invented keys only; no network.
describe('the LMS read client',()=>{
  const principal={teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001'};
  const list=(rows:unknown[],result:unknown='OK')=>JSON.stringify({result,exam_list:rows});

  test('paper_print_exam_list_rows_are_parsed_and_every_sel_type_is_qualified',()=>{
    expect(paperExamListForm('5001')).toEqual({p_process:'getTestExamList',p_no:'5001',p_option_type:'2'});
    for(const bad of ['0','','x','01','1000000000',5 as never])expect(()=>paperExamListForm(bad)).toThrow('paper_no_invalid');
    // `outerLectureKey` is the row's `outer_lecture_key` when the member is present, else null (the exam tools of specs/academy-mcp/gated-writes.md read it; the print path ignores it).
    expect(parsePaperExamList(list([{lecture_key:1234567,outer_lecture_key:100000001,sel_type:'obj',examtext_key:0,pat_name:'Pattern',difficulty:'3',unit_name:'Unit'},
      {lecture_key:'8',outer_lecture_key:'100000002',sel_type:'sbj',examtext_key:'12',pat_name:null},{lecture_key:9,sel_type:'des'}]))).toEqual([
      {lectureKey:1234567,outerLectureKey:100000001,selType:'obj',examtextKey:0,patName:'Pattern',difficulty:'3',unitName:'Unit'},
      {lectureKey:8,outerLectureKey:100000002,selType:'sbj',examtextKey:12,patName:'',difficulty:'',unitName:''},
      {lectureKey:9,outerLectureKey:null,selType:'des',examtextKey:0,patName:'',difficulty:'',unitName:''}]);
    for(const outer of [1000000000,-1,1.5,'x','1e3'])expect(()=>parsePaperExamList(list([{lecture_key:1,outer_lecture_key:outer,sel_type:'obj'}]))).toThrow('paper_exam_list_unverified');
    for(const sel of ['1','OBJ','',' obj',null,1,undefined])
      expect(()=>parsePaperExamList(list([{lecture_key:1,sel_type:'obj'},{lecture_key:2,sel_type:sel}]))).toThrow('paper_exam_sel_type_unqualified');
    for(const [text,code] of [['not json','paper_exam_list_unverified'],[list([{lecture_key:1,sel_type:'obj'}],'FAIL'),'paper_exam_list_unverified'],
      [list([{lecture_key:1,sel_type:'obj'}],'ok'),'paper_exam_list_unverified'],['{"result":"OK"}','paper_exam_list_unverified'],[list([]),'paper_exam_list_empty'],
      [list(Array.from({length:201},()=>({lecture_key:1,sel_type:'obj'}))),'paper_exam_list_too_large'],[list(['x']),'paper_exam_list_unverified'],
      [list([{lecture_key:-1,sel_type:'obj'}]),'paper_exam_list_unverified'],[list([{lecture_key:1.5,sel_type:'obj'}]),'paper_exam_list_unverified'],
      [list([{lecture_key:1000000000,sel_type:'obj'}]),'paper_exam_list_unverified'],[list([{lecture_key:'1e3',sel_type:'obj'}]),'paper_exam_list_unverified'],
      [list([{lecture_key:1,sel_type:'obj',examtext_key:1000000}]),'paper_exam_list_unverified']] as const)
      expect(()=>parsePaperExamList(text)).toThrow(code);
  });

  test('paper_print_exam_list_without_a_session_fails_closed_before_any_request',async()=>{
    const guard=writeForbiddenTransport(()=>new Response('unexpected'));
    const client=new AcademyHttpClient(principal,guard.transport as never);
    await expect(client.readPaperExamList('5001')).rejects.toThrow('authentication_required');
    await expect(client.readPaperExamList('0')).rejects.toThrow('paper_no_invalid');
    expect(guard.requests).toEqual([]);
  });

  test('paper_print_exam_list_wire_posts_the_form_reads_the_rows_and_fails_closed_on_a_login_page',async()=>{
    const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
    const principal='<a href="/logout">Fictional Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="90001"></form><form name="form1"><input name="g_pri_no" value="90001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
    const exam=list([{lecture_key:1234567,sel_type:'obj',examtext_key:0,pat_name:'Invented pattern',difficulty:'3',unit_name:'Invented unit'}]);
    for(const [answer,expected] of [[exam,'rows'],[login,'authentication_required'],['<html>세션이 만료</html>','authentication_required'],[list([{lecture_key:1,sel_type:'1'}]),'paper_exam_sel_type_unqualified']] as const){
      let calls=0;const sent:{url:string;method?:string;body:string;headers:Headers}[]=[];
      const guard=writeForbiddenTransport((url,init)=>{calls++;
        if(calls===1)return new Response(login);if(calls<=3)return new Response(principal);
        sent.push({url,method:init?.method,body:String(init?.body),headers:new Headers(init?.headers)});return new Response(answer);});
      const client=new AcademyHttpClient({teacherLabel:'Fictional Teacher',teacherKey:'90001',collegeKey:'88001'},guard.transport as never);
      await client.authenticate({username:'fictional',password:'fictional'});
      const result=await client.readPaperExamList('5001').then(rows=>({rows}),error=>({error:(error as Error).message}));
      if(expected==='rows')expect(result).toEqual({rows:[{lectureKey:1234567,outerLectureKey:null,selType:'obj',examtextKey:0,patName:'Invented pattern',difficulty:'3',unitName:'Invented unit'}]});
      else expect(result).toEqual({error:expected});
      expect(sent).toHaveLength(1);
      expect(sent[0]!.url).toBe('https://dc.gang-a.kr/servlet/controller.common.TestpageSelectExServlet');
      expect(sent[0]!.method).toBe('POST');
      expect(Object.fromEntries(new URLSearchParams(sent[0]!.body))).toEqual({p_process:'getTestExamList',p_no:'5001',p_option_type:'2'});
      // One read of the exam list, no write.
      expect(guard.requests.filter(r=>r.route.endsWith(' getTestExamList'))).toEqual([{route:'/servlet/controller.common.TestpageSelectExServlet getTestExamList',role:'read'}]);
      expect(guard.forbidden()).toEqual([]);
    }
  });

});

describe('request classes (I1)',()=>{
  test('paper_print_get_test_exam_list_is_a_read_and_the_neighbours_are_not',()=>{
    const path='/servlet/controller.common.TestpageSelectExServlet';
    expect(lmsRequestRole(path,'p_process=getTestExamList&p_no=5001&p_option_type=2')).toBe('read');
    expect(lmsRequestRole(path,'p_process=getTestExamList&p_no=5001&p_option_type=2','POST')).toBe('read');
    expect(lmsRequestRole(path,'p_process=SaveTestExam&p_no=5001')).toBe('write');
    expect(lmsRequestRole(path+'?p_process=getTestExamList')).toBe('read');
    expect(lmsRequestRole('/servlet/controller.common.OtherServlet','p_process=getTestExamList')).toBe('write');
    const guard=writeForbiddenTransport(()=>new Response('{}'));
    return guard.transport('https://dc.gang-a.kr'+path,{method:'POST',body:'p_process=getTestExamList&p_no=5001&p_option_type=2'}).then(r=>{
      expect(r.status).toBe(200);expect(guard.requests).toEqual([{route:path+' getTestExamList',role:'read'}]);
    });
  });
});
