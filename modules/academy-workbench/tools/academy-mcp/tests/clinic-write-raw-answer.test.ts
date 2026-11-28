import {describe,expect,test} from 'bun:test';
import {createHash} from 'node:crypto';
import {AcademyHttpClient} from '../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts';

// specs/academy-mcp/gated-writes.md, "Clinic and wrong-note papers": the write port gives the clinic save the raw bytes and the
// Content-Type of the write answer besides the text it parses. The real client over a recorded synthetic transport (the login
// exchange, then the answer given here); invented values only, 0 live LMS calls.
const TEACHER='99001',COLLEGE='88001',SESSION='invented-session-token-5512';
const loginPage='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd" type="password"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
const principal=`<a href="/logout">Invented Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="${TEACHER}"></form><form name="form1"><input name="g_pri_no" value="${TEACHER}"><input name="g_clg_no" value="${COLLEGE}"><input name="g_mem_type" value="teacher"></form>`;
const SELECT='/servlet/controller.common.TestpageSelectExServlet';
// Not UTF-8: two EUC-KR bytes inside an otherwise ASCII answer.
const RAW=new Uint8Array([...new TextEncoder().encode('{"result":"'),0xb0,0xa1,...new TextEncoder().encode('"}')]);
const CONTENT_TYPE='text/plain; charset=EUC-KR';

async function wire(){
  const seen:{method:string;url:string;body:string}[]=[];
  const transport=async(url:string,init?:RequestInit)=>{
    const u=new URL(url);
    if(u.pathname==='/c_login.jsp')return new Response(loginPage,{headers:{'Set-Cookie':`JSESSIONID=${SESSION}; Path=/`}});
    if(u.pathname==='/servlet/controller.login.LoginServlet'||u.searchParams.get('reqCmd')==='StudyCourse')return new Response(principal);
    seen.push({method:(init?.method??'GET').toUpperCase(),url:u.toString(),body:typeof init?.body==='string'?init.body:''});
    return new Response(RAW.slice(),{headers:{'Content-Type':CONTENT_TYPE}});
  };
  const client=new AcademyHttpClient({teacherLabel:'Invented Teacher',teacherKey:TEACHER,collegeKey:COLLEGE},transport as never);
  await client.authenticate({username:'fictional',password:'fictional'});
  return {client,seen};
}

describe('clinic write port: the raw answer besides the text',()=>{
  test('clinic_client_hands_the_raw_write_answer_besides_the_same_text',async()=>{
    const fields={p_process:'getStudyResultSingleTestingSingleUser',condition:JSON.stringify({testing_no:88001,pri_no:77001,student_name:'',score:{create:0},incorrect:{create:1},similar:{create:0},advance:{create:0},report:{create:0}})};
    const plain=await wire(),text=await plain.client.postTestpageSelectEx({...fields});
    const withRaw=await wire(),given:{contentType:string;raw:Uint8Array;sha256:string}[]=[];
    const same=await withRaw.client.postTestpageSelectEx({...fields},value=>{given.push(value);});
    // The text answered is the one answered without the callback, and the request is the same.
    expect(same).toBe(text);
    expect(withRaw.seen).toEqual(plain.seen);
    expect(withRaw.seen).toHaveLength(1);
    expect(withRaw.seen[0]!.url.endsWith(SELECT)).toBe(true);
    // The callback receives the bytes as sent, their Content-Type and their SHA-256, once.
    expect(given).toHaveLength(1);
    expect(given[0]!.contentType).toBe(CONTENT_TYPE);
    expect(new Uint8Array(given[0]!.raw)).toEqual(RAW);
    expect(given[0]!.sha256).toBe(createHash('sha256').update(RAW).digest('hex'));
    // A callback that fails never changes the text answered.
    const failing=await wire();
    expect(await failing.client.postTestpageSelectEx({...fields},()=>{throw Error('synthetic_callback_failure');})).toBe(text);
  });
});
