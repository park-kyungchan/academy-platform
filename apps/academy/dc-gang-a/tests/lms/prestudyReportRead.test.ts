import {expect,test} from 'bun:test';
import {AcademyHttpClient} from '../../src/lms/academyHttpClient';

// Invented login/principal pages and narratives; transport never uses the network.
const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
const principal='<a href="/logout">Invented Teacher- [로그아웃]</a><form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
const request={teacherLabel:'Invented Teacher',studentName:'Invented Student',studentKey:'77001',prestudyKey:'12345'};
const detail=(key='12345',student='77001',link=true)=>`<form name="form1"><input name="student_pri_no" value="${student}"><input name="prestudy_key" value="${key}"><input name="sunit_name" value="9-1 Invented unit"><input name="create_datetime" value="2040-01-02 03:04:05"><input name="encoding_result" value="success"></form>${link?'<button onclick="viewAiReport(\'55501\')">report</button>':''}`;
const report='<div class="max-w-5xl rounded-2xl"><h1>개념 이해</h1><h3>reason</h3><div class="text-sm">Invented πr² = 50% &lt; 1</div></div>';
function fixture(detailHtml:string){
  const calls:string[]=[];
  const client=new AcademyHttpClient({teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001'},async(url)=>{
    calls.push(url);return new Response(calls.length===1?login:calls.length<=3?principal:calls.length===4?detailHtml:report);
  });
  return {client,calls};
}

test('source reader returns verified detail identity and source literals from exactly one report',async()=>{
  const {client,calls}=fixture(detail());
  try{
    await client.authenticate({username:'inventedUser',password:'inventedPassword'});
    const result=await client.readPrestudyReport(request);
    expect(result).toMatchObject({prestudyKey:'12345',sourceRecord:{prestudyKey:'12345',unitLabel:'9-1 Invented unit',dateLiteral:'2040-01-02 03:04:05'},sections:[{title:'개념 이해',blocks:[{text:'Invented πr² = 50% < 1'}]}]});
    expect(calls).toHaveLength(5);expect(calls[3]).toContain('prestudy_key=12345');expect(calls[4]).toContain('analysis_key=55501');
    expect(JSON.stringify(result)).not.toContain('analysisKey');
  }finally{client.close();}
});

test('wrong returned detail key/student and not-ready records never fetch the report or retry',async()=>{
  for(const [html,error] of [[detail('12346'),'prestudy_detail_record_mismatch'],[detail('12345','77002'),'prestudy_detail_student_mismatch'],[detail('12345','77001',false),'prestudy_report_not_ready']]){
    const {client,calls}=fixture(html);
    try{
      await client.authenticate({username:'inventedUser',password:'inventedPassword'});
      await expect(client.readPrestudyReport(request)).rejects.toThrow(error);
      expect(calls).toHaveLength(4);
    }finally{client.close();}
  }
});
