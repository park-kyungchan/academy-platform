import {describe,expect,test} from 'bun:test';
import {AcademyHttpClient} from '../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts';

// specs/academy-mcp/gated-writes.md R8, R15 and the page's item 12: the client methods of the exam pair send only routes of their own class. The real
// client over a recorded synthetic transport (the login exchange, then the answers given here); invented values only, 0 live LMS calls.
const TEACHER='99001',COLLEGE='88001',SESSION='invented-session-token-4471';
const loginPage='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd" type="password"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
const principal=`<a href="/logout">Invented Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="${TEACHER}"></form><form name="form1"><input name="g_pri_no" value="${TEACHER}"><input name="g_clg_no" value="${COLLEGE}"><input name="g_mem_type" value="teacher"></form>`;
const FA='/servlet/controller.tutor.fa.';

async function wire(answer:(url:URL,init:RequestInit)=>Response=()=>new Response('<root><result>success</result></root>')){
  const seen:{method:string;url:URL;init:RequestInit}[]=[];
  const transport=async(url:string,init?:RequestInit)=>{
    const u=new URL(url);
    if(u.pathname==='/c_login.jsp')return new Response(loginPage,{headers:{'Set-Cookie':`JSESSIONID=${SESSION}; Path=/`}});
    if(u.pathname==='/servlet/controller.login.LoginServlet'||u.searchParams.get('reqCmd')==='StudyCourse')return new Response(principal);
    seen.push({method:(init?.method??'GET').toUpperCase(),url:u,init:init??{}});
    return answer(u,init??{});
  };
  const client=new AcademyHttpClient({teacherLabel:'Invented Teacher',teacherKey:TEACHER,collegeKey:COLLEGE},transport as never);
  return {client,seen,login:()=>client.authenticate({username:'fictional',password:'fictional'})};
}

describe('exam write client: each method sends only routes of its own class',()=>{
  test('exam_client_a_method_refuses_a_route_of_the_other_class_before_any_request',async()=>{
    const w=await wire();await w.login();
    const refuse=async(run:()=>Promise<unknown>)=>expect(run()).rejects.toThrow('exam_route_class_mismatch');
    await refuse(()=>w.client.readExamRoute(`${FA}TestPageRegServlet?p_process=Auto`));                                  // the allocating GET is write
    await refuse(()=>w.client.readExamRoute(`${FA}TestHangServlet?reqCmd=Save&pno=1`));
    await refuse(()=>w.client.readExamRoute('/servlet/controller.common.TestpageSelectServlet?reqCmd=SaveMunhang'));
    await refuse(()=>w.client.readExamRoute(`${FA}TestHangServlet?reqCmd=Modify&pno=1`));                                // not listed: write
    await refuse(()=>w.client.postExamRead(`${FA}TestPageRegServlet?reqCmd=Save`,{reqCmd:'Save'}));
    await refuse(()=>w.client.postExamRead('/servlet/controller.common.ObjectSelectServlet',{reqCmd:'deleteStudent'}));
    await refuse(()=>w.client.writeExamGet(`${FA}TestPageRegServlet?p_process=AutoModify&p_original_testno=1`));         // a read route
    await refuse(()=>w.client.writeExamGet(`${FA}TestHangServlet?reqCmd=Validate&pno=1`));
    await refuse(()=>w.client.writeExamGet(`${FA}TestHangServlet?reqCmd=Del&p_testingno=1`));
    await refuse(()=>w.client.writeExamMultipart(`${FA}TestPageRegServlet?reqCmd=CheckMunhang`,[['a','b']]));
    await refuse(()=>w.client.writeExamMultipart('/servlet/controller.tutor.xx.TestPageRegServlet?reqCmd=Save',[['a','b']]));
    expect(w.seen).toEqual([]);
  });
  test('exam_client_a_route_of_its_class_is_sent_with_the_form_the_page_uses',async()=>{
    const w=await wire((url)=>new Response('answer for '+url.pathname));await w.login();
    expect(await w.client.readExamRoute(`${FA}TestPageRegServlet?p_process=AutoModify&p_original_testno=424001`)).toBe(`answer for ${FA}TestPageRegServlet`);
    expect(await w.client.writeExamGet(`${FA}TestPageRegServlet?p_process=Auto`)).toBe(`answer for ${FA}TestPageRegServlet`);
    await w.client.postExamRead('/servlet/controller.common.ObjectSelectServlet',{reqCmd:'getStudentList',studentName:'Invented Name'});
    await w.client.writeExamMultipart(`${FA}TestPageRegServlet?reqCmd=Save`,[['p_process','Save'],['testname','Invented'],['p_url','a&b=1']]);
    expect(w.seen.map(r=>r.method+' '+r.url.pathname+r.url.search)).toEqual([
      `GET ${FA}TestPageRegServlet?p_process=AutoModify&p_original_testno=424001`,`GET ${FA}TestPageRegServlet?p_process=Auto`,
      'POST /servlet/controller.common.ObjectSelectServlet',`POST ${FA}TestPageRegServlet?reqCmd=Save`]);
    const form=w.seen[2]!.init;
    expect(new Headers(form.headers).get('content-type')).toContain('application/x-www-form-urlencoded');
    expect(Object.fromEntries(new URLSearchParams(form.body as string))).toEqual({reqCmd:'getStudentList',studentName:'Invented Name'});
    // The multipart body is a FormData whose boundary the transport sets: no content-type, no cookie of the caller.
    const multipart=w.seen[3]!.init;
    expect(multipart.body).toBeInstanceOf(FormData);
    expect([...(multipart.body as FormData).entries()]).toEqual([['p_process','Save'],['testname','Invented'],['p_url','a&b=1']]);
    expect(new Headers(multipart.headers).has('content-type')).toBe(false);
    for(const r of w.seen){expect(new Headers(r.init.headers).get('cookie')).toBe('JSESSIONID='+SESSION);expect(r.init.redirect).toBe('manual');}
  });
  test('exam_client_without_a_session_or_with_a_login_answer_it_ends_with_authentication_required',async()=>{
    const cold=await wire();
    await expect(cold.client.readExamRoute(`${FA}TestHangServlet?p_process=Main&p_no=1`)).rejects.toThrow('authentication_required');
    await expect(cold.client.writeExamGet(`${FA}TestPageRegServlet?p_process=Auto`)).rejects.toThrow('authentication_required');
    expect(cold.seen).toEqual([]);
    const expired=await wire(()=>new Response(loginPage));await expired.login();
    expect(expired.client.authenticated).toBe(true);
    await expect(expired.client.readExamRoute(`${FA}TestHangServlet?p_process=Main&p_no=1`)).rejects.toThrow('authentication_required');
    expect(expired.client.authenticated).toBe(false);
    const moved=await wire(()=>new Response('',{status:302,headers:{Location:'/c_login.jsp'}}));await moved.login();
    await expect(moved.client.writeExamGet(`${FA}TestPageRegServlet?p_process=Auto`)).rejects.toThrow('authentication_required');
    expect(moved.client.authenticated).toBe(false);
  });
});
