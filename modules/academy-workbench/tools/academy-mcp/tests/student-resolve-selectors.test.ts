import './network-deny.ts';
import {expect,test} from 'bun:test';
import {AcademyHttpClient} from '../../../../../apps/academy/dc-gang-a/src/lms/academyHttpClient.ts';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';
import {clock,learningPolicy,safeguard} from './dailyrecord-fixture.ts';

// An undated academy_student_resolve through the real LMS client and a fixture
// transport: the sort_date '1' page first, then the history selectors in order.
const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
const principal='<a href="/logout">Invented Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
const selectors=['1','11','12','13','14','15','16'];
const page=(rows:unknown[])=>'<form name="frm"><select name="sort_date">'+selectors.map(v=>'<option value="'+v+'">Invented period '+v+'</option>').join('')
  +"</select></form><script>const x={testResultList:JSON.parse('"+JSON.stringify(rows)+"')};</script>";
const admission={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-04',students:[{studentId:'9101',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
const request={purpose:'Resolve a fictional student',currentRequestRef:'fictional-resolve-request',displayName:'Invented Student'};

function resolver(foundOn:string|null){
  let calls=0;const searched:string[]=[];
  const client=new AcademyHttpClient({teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001'},async(url,init)=>{
    calls++;if(calls<=3)return new Response(calls===1?login:principal);
    if(!String(url).includes('DailyZeroTestServlet?reqCmd=DtResultSearch'))throw Error('unexpected_fixture_request');
    const body=Object.fromEntries(new URLSearchParams(init?.body as string));searched.push(body.sort_date!);
    return new Response(page(body.sort_date===foundOn?[{priNo:9101,userName:'Invented Student'},{priNo:9103,userName:'Other Student'}]:[{priNo:9103,userName:'Other Student'}]));
  });
  const service=new DailyRecordService(new NativeDailyRecordBackend(client as never,admission,()=>({username:'fictional',password:'fictional'})),safeguard,()=>clock,learningPolicy);
  return {service,searched};
}

test('check 1: a name absent on 1 resolves on a later selector, named by fixedSourceFilter, at one request plus its position',async()=>{
  for(const [position,selector] of selectors.slice(1).entries()){
    const {service,searched}=resolver(selector);
    expect(await service.resolveStudent(request)).toEqual({purpose:request.purpose,currentRequestRef:request.currentRequestRef,displayName:'Invented Student',
      academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only',
      status:'resolved',studentKey:'9101',fixedSourceFilter:{sort_date:selector}});
    expect(searched).toEqual(selectors.slice(0,position+2));
    expect(searched).toHaveLength(1+position+1);
  }
});

test('check 2: a name on 1 costs exactly one search request and gives the same result as before',async()=>{
  const {service,searched}=resolver('1');
  expect(await service.resolveStudent(request)).toEqual({purpose:request.purpose,currentRequestRef:request.currentRequestRef,displayName:'Invented Student',
    academyWrites:0,parentSends:0,automaticRetryAllowed:false,sourceContentIsUntrustedData:true,execution:'read_only',
    status:'resolved',studentKey:'9101'});
  expect(searched).toEqual(['1']);
});

test('check 3: a name on no page is student_name_not_found after at most one request per selector',async()=>{
  const {service,searched}=resolver(null);
  expect(await service.resolveStudent(request)).toMatchObject({status:'unavailable',reasonCode:'student_name_not_found',academyWrites:0});
  expect(searched).toEqual(selectors);
  expect(new Set(searched).size).toBe(searched.length);
});
