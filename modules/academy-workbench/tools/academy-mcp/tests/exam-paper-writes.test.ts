import {describe,expect,test} from 'bun:test';
import {chmod,mkdtemp,rm,writeFile} from 'node:fs/promises';
import {existsSync,readFileSync,readdirSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {lmsRequestRole} from './network-deny.ts';
import {ExamPaperService} from '../src/exam-write-tools.ts';
import {
  COURSE_NO,COURSE_PAPER,COOKIE_CANARY,KEY_A,KEY_B,NAME_A,NAME_B,SOURCE_PAPER,CLASS_A,CLASS_B,CLS_NAME_A,CLS_NAME_B,GRADE,FIRST_TESTNO,
  assignmentPage,creationPage,examFixturePort,fixtureExamLms,itemKeys,pageHashes,type FixtureExamLms,
} from './exam-write-fixture-lms.ts';
import {
  ASSIGNMENT_FUNCTIONS,CREATION_FUNCTIONS,EXAM_PAGE_SHA256,FA_FORM_MEMBERS,EXAM_TOOL_FAMILY,expectedExamPageHash,examPageSha256,examScopeKey,examWireSha256,
  parseCheckMunhang,parseCoursePaperNo,parseSaveLogic,parseSaveMunhang,parseValidate,qualifyExamChange,qualifyExamRequest,sortedTargetStrings,xmlValues,
} from '../../../../../apps/academy/dc-gang-a/src/lms/examPaperWrite.ts';
import {GatedWriteLedger,canonicalJson,changeSha256Of,reviewTokenOf,sha256Hex,type AttemptRecord} from '../../../../../apps/academy/dc-gang-a/src/lms/gatedWriteLedger.ts';
import {CLINIC_TOOL_FAMILY} from '../../../../../apps/academy/dc-gang-a/src/lms/clinicPaperWrite.ts';
import {EXAM_READ_ROUTES,EXAM_WRITE_ROUTES,examRequestEffect,examRouteClass} from '../../../../../apps/academy/dc-gang-a/src/lms/lmsRouteRegistry.ts';
import {parsePaperExamList} from '../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead.ts';

// specs/academy-mcp/gated-writes.md R1-R16 for academy_exam_paper_review and academy_exam_paper_save. Invented numeric keys and names, synthetic
// pages whose hashes come through the `expectedWireHashes` seam, and a scratch 0600 ledger: no real student, no live LMS call. The LMS is a recorded
// fixture (tests/exam-write-fixture-lms.ts) that answers write_forbidden (403) to a request classed write while the test has not allowed the writes.
const root=resolve(import.meta.dir,'..'),repo=resolve(root,'../../../..');
const T0=Date.parse('2040-01-16T09:00:00.000Z');
const FA='/servlet/controller.tutor.fa.',DA='/servlet/controller.tutor.da.';
const PERIOD={start:'2040-01-16T09:00',end:'2040-01-16T10:30',testingMinutes:60};
const TARGETS=[{studentName:NAME_A,studentKey:KEY_A},{studentName:NAME_B,studentKey:KEY_B}];
const ITEMS=[{key:'100000001',type:'obj'},{key:'100000002',type:'obj'},{key:'100000003',type:'sbj'},{key:'100000004',type:'des'}];
const faInput=(extra:Record<string,unknown>={})=>({family:'fa',testName:'Invented FA 1',testgubunNo:1180,items:ITEMS,targets:TARGETS,...PERIOD,...extra});
const faPaperInput=(extra:Record<string,unknown>={})=>({family:'fa',testName:'Invented FA 2',testgubunNo:1161,sourcePaperNo:SOURCE_PAPER,targets:TARGETS,...PERIOD,...extra});
const daInput=(extra:Record<string,unknown>={})=>({family:'da',testName:'Invented DA 1',testgubunNo:1180,courseNo:COURSE_NO,targets:TARGETS,...PERIOD,...extra});
const TARGET_STRINGS=[`1!${KEY_A}!${GRADE}!${CLASS_A}`,`1!${KEY_B}!${GRADE}!${CLASS_B}`];

async function scratch(){
  const dir=await mkdtemp(join(tmpdir(),'exam-writes-'));await chmod(dir,0o700);
  return {dir,ledger:join(dir,'ledger.jsonl'),cleanup:()=>rm(dir,{recursive:true,force:true})};
}
function harness(ledger:string|null,options:{lms?:FixtureExamLms;hashes?:boolean;pidAlive?:(pid:number)=>boolean}={}){
  const lms=options.lms??fixtureExamLms(),clock={t:T0};
  let n=0;
  const service=new ExamPaperService(examFixturePort(lms),ledger,{now:()=>clock.t,newId:()=>'id-'+(++n),pidAlive:options.pidAlive,
    ...(options.hashes===false?{}:{expectedWireHashes:pageHashes()})});
  const records=()=>existsSync(ledger??'/nonexistent')?readFileSync(ledger!,'utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l)):[];
  return {lms,clock,service,records,
    attempts:()=>records().filter((r:any)=>r.kind==='attempt') as AttemptRecord[],
    async reviewed(input:Record<string,unknown>){const out:any=await service.review(input);expect(out.status).toBe('reviewed');return out;},
    approve:(review:any,extra:Record<string,unknown>={})=>({reviewToken:review.reviewToken,userApproved:true,change:review.change,...extra}),
    async saved(input:Record<string,unknown>){const review=await this.reviewed(input),out:any=await service.save({reviewToken:review.reviewToken,userApproved:true,change:review.change});return {review,out};}};
}
type Harness=ReturnType<typeof harness>;
const steps=(h:Harness)=>h.attempts().map(a=>`${a.examStep}:${a.step}:${a.state}`);
const last=(h:Harness)=>h.attempts().at(-1)!;
const FULL_STEPS=['A:before_write:open','A:write_dispatched:open','A:response_received:open','A:write_dispatched:open','A:response_received:open','B:write_dispatched:open','B:response_received:open',
  'C:write_dispatched:open','C:response_received:open','C:reconciled:persisted'];
const FULL_ROUTES=(f:'fa'|'da')=>{const reg=`${f==='fa'?FA:DA}TestPageRegServlet`,hang=`${f==='fa'?FA:DA}TestHangServlet`;return [
  `GET ${reg} Auto [write]`,'GET /servlet/controller.common.TestpageSelectServlet SaveMunhang [write]','POST /servlet/controller.common.TestpageSelectExServlet getTestExamList',
  `GET ${reg} CheckMunhang`,`POST ${reg} Save [write]`,`GET ${reg} AutoModify`,`GET ${hang} Main`,`GET ${hang} Validate`,`GET ${hang} Save [write]`,`GET ${hang} Validate`];};
const afterReview=(lms:FixtureExamLms)=>lms.routes().filter(r=>!r.includes('ObjectSelectServlet'));
const hex=(c:string)=>c.repeat(64);
const lockBody=(pid:number,runId='other-run')=>JSON.stringify({pid,runId,at:new Date(T0).toISOString()})+'\n';
/** A run that crashed at the request whose route contains `route`: the fixture throws there, as a dropped connection would. */
const crashAt=(lms:FixtureExamLms,route:string,nth=1)=>{let seen=0;lms.state.onRequest=r=>{if(r.route.includes(route)&&++seen===nth){lms.state.onRequest=null;throw Error('invented_connection_dropped');}};};
const savedAttemptOf=async(h:Harness,input:Record<string,unknown>)=>{const review=await h.reviewed(input);return {review,out:await h.service.save(h.approve(review)).catch((e:Error)=>({thrown:e.message})) as any};};

// ---- the pinned values and the route classes -----------------------------------------------------------
describe('exam writes: pinned wire values and request classes',()=>{
  test('exam_writes_pins_the_four_production_page_hashes_exactly',()=>{
    expect(EXAM_PAGE_SHA256.fa.creation).toBe('7c1ae5a66888e6d4f5aa0b3089bee77dda8e75a3c3c8a4da3b40b0d0f111df1e');
    expect(EXAM_PAGE_SHA256.da.creation).toBe('2c1c72d5a81a117afce22dc1f83c460e9db6b79a6adfdfbb26519d574206af3d');
    expect(EXAM_PAGE_SHA256.fa.assignment).toBe('ed6e161327a83e13a305891dcecd1a0afc4c9b42dbc0eead1bf674c7c2cda7f3');
    expect(EXAM_PAGE_SHA256.da.assignment).toBe('1fa2e96e224d105d39684bcba248b2b6ae8f054bc7dff8388c81eeb0afc20e67');
    expect([...CREATION_FUNCTIONS]).toEqual(['validationTestInfo','requestSave','onSaveButtonClick']);
    expect([...ASSIGNMENT_FUNCTIONS]).toEqual(['validate','onSave','showSaveResult']);
    expect(expectedExamPageHash('fa','creation')).toBe(EXAM_PAGE_SHA256.fa.creation);
    expect(expectedExamPageHash('da','assignment',{da:{assignment:hex('a')}})).toBe(hex('a'));
    expect(expectedExamPageHash('da','creation',{da:{assignment:hex('a')}})).toBe(EXAM_PAGE_SHA256.da.creation);
    // wireSha256 of the token: sha256 of "<family>:" + creation + ":" + assignment.
    expect(examWireSha256('fa')).toBe(sha256Hex(`fa:${EXAM_PAGE_SHA256.fa.creation}:${EXAM_PAGE_SHA256.fa.assignment}`));
    expect(examWireSha256('da')).toBe(sha256Hex(`da:${EXAM_PAGE_SHA256.da.creation}:${EXAM_PAGE_SHA256.da.assignment}`));
    // The synthetic pages never carry the production hashes.
    expect(pageHashes().fa.creation).not.toBe(EXAM_PAGE_SHA256.fa.creation);
  });
  test('exam_writes_the_page_hash_is_over_the_functions_in_the_order_listed_with_line_ends_normalized',()=>{
    const page=creationPage('fa',1);
    expect(examPageSha256('creation',page)).toBe(pageHashes().fa.creation);
    expect(examPageSha256('creation',page.replaceAll('\r\n','\n'))).toBe(pageHashes().fa.creation);
    expect(examPageSha256('creation',page.replace('function requestSave(a) {\n  var v = 2;','function requestSave(a) {\n  var v = 3;'))).not.toBe(pageHashes().fa.creation);
    expect(examPageSha256('creation',page.replace('onSaveButtonClick','somethingElse'))).toBeNull();
    expect(examPageSha256('creation',page+page)).toBeNull();
    expect(examPageSha256('creation',assignmentPage())).toBeNull();
    expect(examPageSha256('assignment',assignmentPage())).toBe(pageHashes().fa.assignment);
  });
  test('exam_writes_without_the_test_seam_a_synthetic_page_is_a_wire_hash_mismatch',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger,{hashes:false});
      const review=await h.reviewed(faInput());                        // the review reads no page: its token holds the pinned production hashes
      expect(review.wireSha256).toBe(examWireSha256('fa'));
      expect(await h.service.save(h.approve(review))).toMatchObject({status:'refused',code:'wire_hash_mismatch',academyWrites:1,effectStatus:'absent'});
      expect(h.lms.writes()).toHaveLength(1);
    }finally{await s.cleanup();}
  });
  test('exam_writes_route_classes_read_and_write_by_route_never_by_method',()=>{
    const reg=(f:string)=>`/servlet/controller.tutor.${f}.TestPageRegServlet`,hang=(f:string)=>`/servlet/controller.tutor.${f}.TestHangServlet`;
    const table:[string,string,string,'read'|'write'][]=[];
    for(const f of ['fa','da']){
      table.push(['GET',reg(f),'Auto','write'],['GET',reg(f),'CheckMunhang','read'],['GET',reg(f),'AutoModify','read'],['POST',reg(f),'Save','write'],
        ['GET',hang(f),'Main','read'],['GET',hang(f),'Validate','read'],['GET',hang(f),'Save','write']);
    }
    table.push(['GET','/servlet/controller.common.TestpageSelectServlet','SaveMunhang','write'],['POST','/servlet/controller.common.ObjectSelectServlet','getStudentList','read'],
      ['POST','/servlet/controller.tutor.diag.DiagnostManageServlet','GetPaperNo','read'],['POST','/servlet/controller.common.TestpageSelectExServlet','getTestExamList','read'],
      ['GET','/servlet/controller.tutor.base.TestPageListServlet','UserByMain','read'],['POST','/servlet/controller.tutor.base.TestPageListServlet','UserBySearchTestResult','read']);
    for(const [method,path,operation,expected] of table){
      expect({method,path,operation,listed:examRouteClass(method,path,operation),effect:examRequestEffect(method,path,operation)}).toEqual({method,path,operation,listed:expected,effect:expected});
      // The fixture's classifier (tests/network-deny.ts) is the same table.
      const form=method==='POST'?new URLSearchParams(operation==='getTestExamList'?{p_process:operation}:{reqCmd:operation}).toString():undefined;
      expect({path,operation,denyTable:lmsRequestRole('https://dc.gang-a.kr'+path+(method==='GET'?`?reqCmd=${operation}`:''),form,method)}).toEqual({path,operation,denyTable:expected});
    }
    // Not listed: write (a GET of a write route, a read operation of the wrong servlet or method, the deferred and unbuilt operations).
    for(const [method,path,operation] of [['GET',reg('fa'),'Modify'],['GET',hang('fa'),'Modify'],['GET',hang('fa'),'Del'],['GET',hang('da'),'ApplyDel'],['GET',hang('fa'),'PreprocessForAutoScore'],
      ['GET',reg('fa'),'UpdateSave'],['POST',reg('fa'),'Auto'],['GET',reg('fa'),'Save'],['POST',hang('fa'),'Validate'],['GET','/servlet/controller.tutor.fa.TestPageListGrpServlet','applyTest'],
      ['GET','/servlet/controller.common.TestpageSelectServlet','getStudentList'],['GET','/servlet/controller.tutor.xx.TestHangServlet','Main']] as const){
      expect({method,path,operation,listed:examRouteClass(method,path,operation),effect:examRequestEffect(method,path,operation)}).toEqual({method,path,operation,listed:null,effect:'write'});
    }
    expect(EXAM_WRITE_ROUTES).toHaveLength(7);
    expect(EXAM_WRITE_ROUTES.map(r=>r.operation).sort()).toEqual(['Auto','Auto','Save','Save','Save','Save','SaveMunhang']);
    expect(EXAM_READ_ROUTES.every(r=>r.path!=='/servlet/controller.tutor.fa.TestPageListGrpServlet')).toBe(true);
  });
});

// ---- the review input: found from the input alone, 0 HTTP requests ---------------------------------------
describe('exam writes R7, R15: a review input is qualified before any request',()=>{
  test('exam_writes_r7_class_grade_and_academy_wide_targets_are_refused_with_0_requests',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger);h.lms.state.allowWrite=false;
      const cases:[string,Record<string,unknown>,string][]=[
        ['class iden 2',faInput({targets:[{studentName:NAME_A,studentKey:KEY_A,iden:2}]}),'class_target_deferred'],
        ['class member',faInput({targets:[{classKey:'5001'}]}),'class_target_deferred'],
        ['class at the top',faInput({classKey:'5001'}),'class_target_deferred'],
        ['group at the top',faInput({groupKey:'5001'}),'class_target_deferred'],
        ['grade iden 3',faInput({targets:[{studentName:NAME_A,studentKey:KEY_A,iden:3}]}),'public_scope_forbidden'],
        ['grade member',faInput({targets:[{gradeNo:'2'}]}),'public_scope_forbidden'],
        ['grade at the top',faInput({gradeNo:'2'}),'public_scope_forbidden'],
        ['all students',faInput({allStudents:true}),'public_scope_forbidden'],
        ['academy wide',faInput({academyWide:true}),'public_scope_forbidden'],
        ['school wide',faInput({schoolWide:true}),'public_scope_forbidden'],
        ['public pool',faInput({publicPool:true}),'public_scope_forbidden'],
        ['private_chk 0',faInput({private_chk:0}),'public_scope_forbidden'],
        ['private_chk 2',faInput({private_chk:'2'}),'public_scope_forbidden'],
        ['no targets',faInput({targets:undefined}),'public_scope_forbidden'],
        ['empty targets',faInput({targets:[]}),'public_scope_forbidden'],
        ['targets not a list',faInput({targets:'all'}),'public_scope_forbidden'],
        ['unknown iden',faInput({targets:[{studentName:NAME_A,studentKey:KEY_A,iden:4}]}),'tool_input_invalid'],
      ];
      for(const [label,input,code] of cases){
        expect({label,out:await h.service.review(input)}).toMatchObject({label,out:{status:'refused',code,academyWrites:0}});
      }
      // private_chk 1 is the only value, and it is not part of the change.
      expect(await h.service.review(faInput({private_chk:1}))).toMatchObject({status:'reviewed'});
      expect(h.lms.requests.filter(r=>r.route.includes('Auto')||r.method==='POST'&&!r.route.includes('ObjectSelect'))).toEqual([]);
    }finally{await s.cleanup();}
  });
  test('exam_writes_a_class_target_member_in_the_save_change_is_refused_too',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.requests.length=0;
      const change=(over:Record<string,unknown>)=>({...review.change,...over});
      const target=(over:Record<string,unknown>)=>({...review.change.targets[0],...over});
      for(const [change_,code] of [[change({classKey:'5001'}),'class_target_deferred'],[change({targets:[target({iden:2})]}),'class_target_deferred'],[change({targets:[target({iden:3})]}),'public_scope_forbidden'],
        [change({allStudents:true}),'public_scope_forbidden'],[change({private_chk:0}),'public_scope_forbidden'],[change({targets:[]}),'public_scope_forbidden']] as const){
        expect({code,out:await h.service.save(h.approve(review,{change:change_}))}).toMatchObject({code,out:{status:'refused',code,academyWrites:0}});
      }
      expect(h.lms.requests).toEqual([]);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r15_an_input_that_does_not_fit_is_refused_with_0_requests',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger);h.lms.state.allowWrite=false;
      const many=(n:number,type:'obj'|'sbj'|'des'='obj')=>itemKeys(n).map(key=>({key,type}));
      const cases:[string,Record<string,unknown>,string][]=[
        ['family',faInput({family:'na'}),'tool_input_invalid'],['testName empty',faInput({testName:''}),'tool_input_invalid'],['testName 51',faInput({testName:'x'.repeat(51)}),'tool_input_invalid'],
        ['testName padded',faInput({testName:' x'}),'tool_input_invalid'],['testName control',faInput({testName:'a\u0007b'}),'tool_input_invalid'],
        ['testgubun -1',faInput({testgubunNo:-1}),'tool_input_invalid'],['testgubun text',faInput({testgubunNo:'1180'}),'tool_input_invalid'],['testgubun missing',faInput({testgubunNo:undefined}),'tool_input_invalid'],
        ['both sources',faInput({sourcePaperNo:SOURCE_PAPER}),'tool_input_invalid'],['no source',faInput({items:undefined}),'tool_input_invalid'],
        ['fa with courseNo',faInput({courseNo:COURSE_NO}),'tool_input_invalid'],['da with items',daInput({items:ITEMS}),'tool_input_invalid'],['da with sourcePaperNo',daInput({sourcePaperNo:SOURCE_PAPER}),'tool_input_invalid'],
        ['da without courseNo',daInput({courseNo:undefined}),'tool_input_invalid'],['sourcePaperNo bad',faPaperInput({sourcePaperNo:'12a'}),'tool_input_invalid'],
        ['no items',faInput({items:[]}),'tool_input_invalid'],['item key bad',faInput({items:[{key:'12a',type:'obj'}]}),'tool_input_invalid'],['item key long',faInput({items:[{key:'1234567890',type:'obj'}]}),'tool_input_invalid'],
        ['item type',faInput({items:[{key:'1',type:'essay'}]}),'tool_input_invalid'],['item member',faInput({items:[{key:'1',type:'obj',extra:1}]}),'tool_input_invalid'],
        ['item duplicate',faInput({items:[{key:'1',type:'obj'},{key:'1',type:'obj'}]}),'tool_input_invalid'],
        ['order sbj before obj',faInput({items:[{key:'1',type:'sbj'},{key:'2',type:'obj'}]}),'invalid_item_order'],['order des before sbj',faInput({items:[{key:'1',type:'des'},{key:'2',type:'sbj'}]}),'invalid_item_order'],
        ['31 items',faInput({items:many(31)}),'too_many_items'],
        ['studentName long',faInput({targets:[{studentName:'x'.repeat(21),studentKey:KEY_A}]}),'tool_input_invalid'],['studentKey text',faInput({targets:[{studentName:NAME_A,studentKey:'a1'}]}),'tool_input_invalid'],
        ['studentKey zero',faInput({targets:[{studentName:NAME_A,studentKey:'0'}]}),'tool_input_invalid'],['target member',faInput({targets:[{studentName:NAME_A}]}),'tool_input_invalid'],
        ['duplicate student',faInput({targets:[TARGETS[0],TARGETS[0]]}),'tool_input_invalid'],
        ['41 targets',faInput({targets:Array.from({length:41},(_,i)=>({studentName:'S'+i,studentKey:String(80000+i)}))}),'tool_input_invalid'],
        ['start format',faInput({start:'2040-01-16 09:00'}),'tool_input_invalid'],['start impossible',faInput({start:'2040-02-30T09:00'}),'tool_input_invalid'],['hour 24',faInput({start:'2040-01-16T24:00'}),'tool_input_invalid'],
        ['start after end',faInput({start:'2040-01-16T11:00'}),'tool_input_invalid'],['start equals end',faInput({end:'2040-01-16T09:00'}),'tool_input_invalid'],
        ['minutes 0',faInput({testingMinutes:0}),'tool_input_invalid'],['minutes 301',faInput({testingMinutes:301}),'tool_input_invalid'],['minutes text',faInput({testingMinutes:'60'}),'tool_input_invalid'],
        ['unknown member',faInput({colour:'red'}),'tool_input_invalid'],
      ];
      for(const [label,input,code] of cases){
        expect({label,out:await h.service.review(input)}).toMatchObject({label,out:{status:'refused',code,academyWrites:0}});
      }
      expect(h.lms.requests).toEqual([]);
      // 30 items and an item list in the exact OMR order pass.
      expect(qualifyExamRequest(faInput({items:many(30)}))).toBeTruthy();
      expect(qualifyExamRequest(faInput({items:[{key:'1',type:'obj'},{key:'2',type:'sbj'},{key:'3',type:'sbj'},{key:'4',type:'des'}]}))).toBeTruthy();
    }finally{await s.cleanup();}
  });
});

// ---- R1, R2: review ------------------------------------------------------------------------------------
describe('exam writes R1, R2: the review',()=>{
  test('exam_writes_r1_review_sends_only_read_requests_and_answers_a_preview_without_other_student_data',async()=>{
    const s=await scratch();
    try{
      const lms=fixtureExamLms();lms.state.allowWrite=false;
      const h=harness(s.ledger,{lms});
      const review=await h.reviewed(faInput());
      expect(lms.routes()).toEqual(Array(2).fill('POST /servlet/controller.common.ObjectSelectServlet getStudentList'));
      expect(lms.requests.map(r=>r.form.get('studentName'))).toEqual([NAME_A,NAME_B]);
      expect(lms.writes()).toEqual([]);
      expect(review).toMatchObject({status:'reviewed',family:'fa',academyWrites:0,wireSha256:examWireSha256('fa',pageHashes()),
        change:{steps:'ABC',family:'fa',testName:'Invented FA 1',testgubunNo:1180,items:ITEMS,...PERIOD,
          targets:[{studentName:NAME_A,studentKey:KEY_A,gradeNo:GRADE,clsNo:CLASS_A},{studentName:NAME_B,studentKey:KEY_B,gradeNo:GRADE,clsNo:CLASS_B}]}});
      expect(review.preview).toEqual({family:'fa',testName:'Invented FA 1',testgubunNo:1180,itemCounts:{obj:2,sbj:1,des:1},steps:'ABC',
        targets:[{studentName:NAME_A,clsName:CLS_NAME_A},{studentName:NAME_B,clsName:CLS_NAME_B}],period:{...PERIOD}});
      expect(JSON.stringify(review)).not.toContain('invented-login');
      expect(review.expiresAt).toBe(new Date(T0+15*60*1000).toISOString());
      expect(review.reviewToken).toMatch(/^[a-f0-9]{64}$/);
      // The review does not read the allocating Auto page.
      expect(lms.requests.some(r=>r.route.includes('Auto'))).toBe(false);
      expect(lms.state.allocated).toEqual([]);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r2_the_review_token_is_the_sha256_of_the_canonical_change_targets_scope_wire_hash_and_expiry',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger);
      const first=await h.reviewed(faInput()),again=await h.reviewed(faInput());
      const scopeKey='fa:'+sha256Hex(TARGET_STRINGS.join('\n'));
      expect(first.scopeKey).toBe(scopeKey);
      expect(examScopeKey('fa',first.change.targets)).toBe(scopeKey);
      expect(sortedTargetStrings(first.change.targets)).toEqual(TARGET_STRINGS);
      expect(again.reviewToken).toBe(first.reviewToken);
      expect(first.reviewToken).toBe(reviewTokenOf({tool:EXAM_TOOL_FAMILY,change:first.change,targets:TARGET_STRINGS,scopeKey,wireSha256:first.wireSha256,expiresAt:first.expiresAt}));
      expect(first.reviewToken).toBe(sha256Hex(canonicalJson({tool:'academy_exam_paper',change:first.change,targets:TARGET_STRINGS,scopeKey,wireSha256:first.wireSha256,expiresAt:first.expiresAt})));
      h.clock.t+=1000;
      expect((await h.reviewed(faInput())).reviewToken).not.toBe(first.reviewToken);
      expect((await h.reviewed(faInput({testName:'Invented FA 9'}))).reviewToken).not.toBe(first.reviewToken);
      // The ledger holds keys and hashes only: no studentKey member, the sorted target strings, no name.
      const reviewed=h.records().find((r:any)=>r.kind==='reviewed');
      expect(reviewed).toEqual({kind:'reviewed',tool:EXAM_TOOL_FAMILY,reviewToken:first.reviewToken,changeSha256:changeSha256Of(first.change),targets:TARGET_STRINGS,scopeKey,
        wireSha256:first.wireSha256,reviewedAt:new Date(T0).toISOString(),expiresAt:first.expiresAt});
      // The order of the targets in the input does not change the token.
      const swapped=await h.reviewed(faInput({targets:[...TARGETS].reverse()}));
      h.clock.t=T0;
      expect(swapped.change.targets).toEqual(first.change.targets);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r1_fa_review_of_a_source_paper_copies_its_items_by_outer_lecture_key',async()=>{
    const s=await scratch();
    try{
      const lms=fixtureExamLms();lms.state.allowWrite=false;
      const h=harness(s.ledger,{lms});
      const review=await h.reviewed(faPaperInput());
      expect(lms.routes().slice(2)).toEqual(['POST /servlet/controller.common.TestpageSelectExServlet getTestExamList']);
      expect(lms.requests.at(-1)!.form.get('p_no')).toBe(SOURCE_PAPER);
      expect(review.change).toMatchObject({sourcePaperNo:SOURCE_PAPER,items:[{key:'100000001',type:'obj'},{key:'100000002',type:'obj'},{key:'100000003',type:'sbj'},{key:'100000004',type:'des'}]});
      expect(review.preview.itemCounts).toEqual({obj:2,sbj:1,des:1});
      // More than 30 items, a row without outer_lecture_key, an empty paper and an out-of-order paper.
      const refuse=async(rows:any[]|null,code:string)=>{
        const l=fixtureExamLms();if(rows)l.state.sourcePapers.set(SOURCE_PAPER,rows);else l.state.sourcePapers.delete(SOURCE_PAPER);
        expect(await harness(s.ledger,{lms:l}).service.review(faPaperInput())).toMatchObject({status:'refused',code,academyWrites:0});
        expect(l.writes()).toEqual([]);
      };
      const row=(k:number,t='obj')=>({sel_type:t,outer_lecture_key:k,lecture_key:5000+k});
      await refuse(itemKeys(31).map(k=>row(Number(k))),'too_many_items');
      await refuse([row(100000001),{sel_type:'obj',lecture_key:5}] as any,'paper_exam_list_unverified');
      await refuse(null,'paper_exam_list_empty');
      await refuse([row(100000001,'sbj'),row(100000002,'obj')],'invalid_item_order');
    }finally{await s.cleanup();}
  });
  test('exam_writes_r1_da_review_reads_the_course_paper_and_needs_exactly_30_items',async()=>{
    const s=await scratch();
    try{
      const lms=fixtureExamLms();lms.state.allowWrite=false;
      const h=harness(s.ledger,{lms});
      const review=await h.reviewed(daInput());
      expect(lms.routes().slice(2)).toEqual(['POST /servlet/controller.tutor.diag.DiagnostManageServlet GetPaperNo','POST /servlet/controller.common.TestpageSelectExServlet getTestExamList']);
      expect(Object.fromEntries(lms.requests[2]!.form)).toEqual({reqCmd:'GetPaperNo',assNo:'1000',courseNo:String(COURSE_NO),level_type:'0'});
      expect(lms.requests[3]!.form.get('p_no')).toBe(COURSE_PAPER);
      expect(review.change).toMatchObject({steps:'ABC',family:'da',courseNo:COURSE_NO});
      expect(review.change.items).toHaveLength(30);
      expect(review.change.sourcePaperNo).toBeUndefined();
      expect(review.preview.itemCounts).toEqual({obj:20,sbj:6,des:4});
      // level_type is 1 exactly when testgubunNo is 1320.
      const l2=fixtureExamLms();await harness(s.ledger,{lms:l2}).service.review(daInput({testgubunNo:1320}));
      expect(l2.requests[2]!.form.get('level_type')).toBe('1');
      // pNo missing, 0, not digits, an unreadable answer; and a paper with other than 30 items.
      for(const answer of [JSON.stringify({result:'OK'}),JSON.stringify({pNo:0,result:'OK'}),JSON.stringify({pNo:'12a',result:'OK'}),JSON.stringify({pNo:'',result:'OK'}),'not json','[]']){
        const l=fixtureExamLms();l.state.coursePaperAnswer=()=>answer;
        expect({answer,out:await harness(s.ledger,{lms:l}).service.review(daInput())}).toMatchObject({answer,out:{status:'refused',code:'course_paper_missing',academyWrites:0}});
        expect(l.requests.some(r=>r.route.includes('getTestExamList'))).toBe(false);
      }
      const l3=fixtureExamLms();l3.state.sourcePapers.set(COURSE_PAPER,l3.state.sourcePapers.get(COURSE_PAPER)!.slice(0,29));
      expect(await harness(s.ledger,{lms:l3}).service.review(daInput())).toMatchObject({status:'refused',code:'da_item_count'});
    }finally{await s.cleanup();}
  });
  test('exam_writes_r1_a_target_is_exactly_one_row_of_the_student_search',async()=>{
    const s=await scratch();
    try{
      const refuse=async(setup:(l:FixtureExamLms)=>void,code:string,input:Record<string,unknown>=faInput())=>{
        const l=fixtureExamLms();setup(l);
        expect({code,out:await harness(s.ledger,{lms:l}).service.review(input)}).toMatchObject({code,out:{status:'refused',code,academyWrites:0}});
        expect(l.writes()).toEqual([]);
      };
      await refuse(()=>{},'target_not_found',faInput({targets:[{studentName:NAME_A,studentKey:'79999'}]}));            // the key is another student's
      await refuse(()=>{},'target_not_found',faInput({targets:[{studentName:'Invented Nobody',studentKey:KEY_A}]}));   // no row
      await refuse(l=>{l.state.students.push({...l.state.students[0]!});},'target_not_found',faInput({targets:[TARGETS[0]!]})); // two rows
      for(const shape of ['not json','[]',JSON.stringify({result:'ERR',student_list:[]}),JSON.stringify({result:'OK'}),JSON.stringify({result:'OK',student_list:[1]}),
        JSON.stringify({result:'OK',student_list:[{student_name:NAME_A,pri_no:Number(KEY_A),cls_name:'x'}]})])
        await refuse(l=>{l.state.studentAnswer=()=>shape;},'target_lookup_unreadable');
    }finally{await s.cleanup();}
  });
});

// ---- R3 to R6: the save gate ---------------------------------------------------------------------------
describe('exam writes R3 to R6: the save gate',()=>{
  test('exam_writes_r3_save_without_user_approval_true_is_refused_with_0_requests',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.requests.length=0;
      for(const userApproved of [undefined,false,'true',1,null,'yes'])
        expect(await h.service.save({reviewToken:review.reviewToken,change:review.change,...(userApproved===undefined?{}:{userApproved})})).toMatchObject({status:'refused',code:'user_approval_required',academyWrites:0});
      expect(h.lms.requests).toEqual([]);
      expect(h.attempts()).toEqual([]);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r4_an_unknown_used_foreign_or_changed_token_is_review_token_invalid_with_0_requests',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput()),other=await h.reviewed(faInput({testName:'Invented FA 7'}));
      h.lms.requests.length=0;
      const refuse=async(input:Record<string,unknown>)=>expect(await h.service.save(input)).toMatchObject({status:'refused',code:'review_token_invalid',academyWrites:0});
      await refuse(h.approve(review,{reviewToken:hex('a')}));                                       // unknown
      await refuse(h.approve(review,{reviewToken:undefined}));                                      // missing
      await refuse(h.approve(other,{reviewToken:review.reviewToken}));                              // another review's change
      await refuse(h.approve(review,{change:{...review.change,testName:'Invented FA 8'}}));
      await refuse(h.approve(review,{change:{...review.change,testgubunNo:1161}}));
      await refuse(h.approve(review,{change:{...review.change,items:[...review.change.items].reverse()}}));
      await refuse(h.approve(review,{change:{...review.change,items:review.change.items.slice(1)}}));
      await refuse(h.approve(review,{change:{...review.change,targets:review.change.targets.slice(1)}}));
      await refuse(h.approve(review,{change:{...review.change,targets:[{...review.change.targets[0],studentName:'Invented Other'},review.change.targets[1]]}}));
      await refuse(h.approve(review,{change:{...review.change,end:'2040-01-16T11:30'}}));
      await refuse(h.approve(review,{change:{...review.change,testingMinutes:61}}));
      await refuse(h.approve(review,{change:{...review.change,family:'da',courseNo:COURSE_NO}}));
      await refuse(h.approve(review,{change:{...review.change,extra:1}}));
      await refuse(h.approve(review,{change:'x'}));
      // A token made by the clinic family is another family's token (a record with its own tool, studentKey and scope).
      new GatedWriteLedger(s.ledger).append({kind:'reviewed',tool:CLINIC_TOOL_FAMILY,reviewToken:hex('c'),changeSha256:hex('d'),studentKey:KEY_A,scopeKey:'clinic:88001',wireSha256:hex('e'),reviewedAt:new Date(T0).toISOString(),expiresAt:new Date(T0+1000).toISOString()});
      await refuse(h.approve(review,{reviewToken:hex('c')}));
      expect(h.lms.requests).toEqual([]);
      // A token is used once: the first save runs, the second is review_token_invalid.
      expect(await h.service.save(h.approve(review))).toMatchObject({status:'saved'});
      h.lms.requests.length=0;
      await refuse(h.approve(review));
      expect(h.lms.requests).toEqual([]);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r5_a_token_more_than_15_minutes_old_is_review_token_expired_with_0_requests',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.requests.length=0;
      h.clock.t=T0+15*60*1000+1;
      expect(await h.service.save(h.approve(review))).toMatchObject({status:'refused',code:'review_token_expired',academyWrites:0});
      expect(h.lms.requests).toEqual([]);
      h.clock.t=T0+15*60*1000;
      expect(await h.service.save(h.approve(review))).toMatchObject({status:'saved'});
    }finally{await s.cleanup();}
  });
  test('exam_writes_r6_a_token_whose_pinned_hashes_changed_since_the_review_refuses_before_any_request',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.requests.length=0;
      const other=new ExamPaperService(examFixturePort(h.lms),s.ledger,{now:()=>T0,newId:()=>'x',expectedWireHashes:{fa:{creation:hex('9'),assignment:pageHashes().fa.assignment}}});
      expect(await other.save(h.approve(review))).toMatchObject({status:'refused',code:'wire_hash_mismatch',academyWrites:0});
      expect(h.lms.requests).toEqual([]);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r10_an_unset_ledger_is_ledger_unavailable_before_any_request',async()=>{
    const lms=fixtureExamLms();
    for(const path of [null,'','relative.jsonl']){
      const h=harness(path,{lms});
      expect(await h.service.review(faInput())).toMatchObject({status:'refused',code:'ledger_unavailable',academyWrites:0});
      expect(await h.service.save({reviewToken:hex('a'),userApproved:true,change:{}})).toMatchObject({status:'refused'});
    }
    expect(lms.requests).toEqual([]);
  });
});

// ---- R8, R14: the save, step by step ------------------------------------------------------------------
describe('exam writes R8, R14: the save of a paper and its assignment',()=>{
  test('exam_writes_r8_a_fa_save_sends_the_allocating_get_then_the_three_steps_and_reads_each_back',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.requests.length=0;
      const out=await h.service.save(h.approve(review)) as any;
      expect(out).toMatchObject({status:'saved',family:'fa',testno:String(FIRST_TESTNO),steps:'ABC',effectStatus:'persisted',academyWrites:4,parentSends:0,automaticRetryAllowed:false});
      expect(h.lms.routes()).toEqual(FULL_ROUTES('fa'));
      expect(h.lms.writes()).toHaveLength(4);
      expect(steps(h)).toEqual(FULL_STEPS);
      const testno=String(FIRST_TESTNO),by=(route:string,n=0)=>h.lms.requests.filter(r=>r.route.includes(route))[n]!;
      // Step A: the allocating GET, SaveMunhang with the items in the OMR order, then the readback of the staged list.
      expect(by('Auto').path+'?'+by('Auto').query).toBe(`${FA}TestPageRegServlet?p_process=Auto`);
      const stage=by('SaveMunhang').query;
      expect(Object.fromEntries(stage)).toEqual({reqCmd:'SaveMunhang',p_original_testno:testno,ass_no:'1001',keyword_title:'Invented FA 1',
        exam_list:'100000001|_|1|,|100000002|_|1|,|100000003|_|2|,|100000004|_|3'});
      expect(by('getTestExamList').form.get('p_no')).toBe(testno);
      // Step B: CheckMunhang with the staged totals, then the multipart paper.
      expect(Object.fromEntries(by('CheckMunhang').query)).toEqual({reqCmd:'CheckMunhang',param:testno,param1:'0',s_cnt:'1',o_cnt:'2',d_cnt:'1',total:'4',bunit1:'11',munit1:'12',bunit2:'13',munit2:'14',
        o_views:'invented-views',param2:'0',uniq:String(T0)});
      const save=h.lms.requests.find(r=>r.method==='POST'&&r.route.endsWith(' Save'))!,members=Object.fromEntries(save.multipart!);
      expect(save.method).toBe('POST');expect(save.path+'?'+save.query).toBe(`${FA}TestPageRegServlet?reqCmd=Save`);
      expect(members).toMatchObject({p_process:'Save',testname:'Invented FA 1',keyword_title:'Invented FA 1',title_option:'',testgubun:'1180',private_chk:'1',total:'4',object:'2',subjectquestion:'1',
        descriptquestion:'1',grade:'23',subject:'22',book:'21',bunit1:'11',munit1:'12',bunit2:'13',munit2:'14',p_original_testno:testno,makecollege:'501',
        evalgubun:'1001',p_url:'a&b=1',makedate:'2040-01-16',objectviews:'invented-views',exam_test:'x-exam_test'});
      expect(Object.keys(members)).toHaveLength(FA_FORM_MEMBERS.length);                 // every form1 member the page serves, once
      expect(save.multipart!.filter(([n])=>n==='courseNo'||n==='useterm')).toEqual([]);
      expect(by('AutoModify').query.get('p_original_testno')).toBe(testno);
      // Step C: the assignment page, Validate and Save with the same members, the readback Validate.
      expect(by('Main').query.get('p_no')).toBe(testno);
      const save1=by('TestHangServlet Save'),validate0=by('Validate',0),validate1=by('Validate',1);
      const member=(q:URLSearchParams)=>[...q].filter(([k])=>k!=='reqCmd');
      expect(member(validate0.query)).toEqual(member(save1.query));expect(member(validate1.query)).toEqual(member(save1.query));
      expect(Object.fromEntries(save1.query)).toEqual({reqCmd:'Save',objecter:TARGET_STRINGS.map(t=>t+'-').join(''),title:'Invented FA 1',name:'Invented FA 1',munhangCount:'4',
        sdate:'2040-1-16',shour:'9',sminute:'0',edate:'2040-1-16',ehour:'10',eminute:'30',testingTime:'60',assYearMonth:'2040-01',applyType:'1',pno:testno,testingno:'',examPlayType:'0',dummy:String(T0)});
      expect(validate0.query.get('reqCmd')).toBe('Validate');
      // The recorded LMS holds the paper and its assignment, and no second paper was allocated.
      expect(h.lms.state.saved.get(testno)).toEqual({testname:'Invented FA 1'});
      expect(h.lms.state.assigned.get(testno)).toEqual([NAME_A,NAME_B]);
      expect(h.lms.state.allocated).toEqual([FIRST_TESTNO]);
      // The ledger holds keys and hashes only; every record of the call carries the sorted targets and, once known, the testno.
      for(const a of h.attempts()){expect(a.studentKey).toBeUndefined();expect(a.targets).toEqual(TARGET_STRINGS);expect(a.scopeKey).toBe(review.scopeKey);}
      expect(h.attempts().map(a=>a.testno)).toEqual([undefined,undefined,testno,testno,testno,testno,testno,testno,testno,testno]);
      const ledgerText=readFileSync(s.ledger,'utf8');
      for(const secret of [NAME_A,NAME_B,'Invented FA 1',COOKIE_CANARY,'invented-login'])expect(ledgerText).not.toContain(secret);
      expect(new Set(h.attempts().map(a=>a.attemptId)).size).toBe(1);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r8_a_da_save_adds_the_course_and_keeps_the_page_values',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(daInput());
      h.lms.requests.length=0;
      const out=await h.service.save(h.approve(review)) as any;
      expect(out).toMatchObject({status:'saved',family:'da',academyWrites:4,effectStatus:'persisted'});
      expect(h.lms.routes()).toEqual(FULL_ROUTES('da'));
      const by=(route:string)=>h.lms.requests.find(r=>r.route.includes(route))!;
      expect(by('SaveMunhang').query.get('ass_no')).toBe('1000');
      expect(by('SaveMunhang').query.get('exam_list')!.split('|,|')).toHaveLength(30);
      expect(by('CheckMunhang').query.get('param2')).toBe('1');
      const save=h.lms.requests.find(r=>r.method==='POST'&&r.route.endsWith(' Save'))!,members=Object.fromEntries(save.multipart!);
      expect(save.path+'?'+save.query).toBe(`${DA}TestPageRegServlet?reqCmd=Save&courseNo=${COURSE_NO}`);
      expect(members).toMatchObject({courseNo:String(COURSE_NO),useterm:'2',evalgubun:'1000',private_chk:'1',total:'30',object:'20',subjectquestion:'6',descriptquestion:'4',testgubun:'1180',makecollege:'501'});
      expect(save.multipart!.filter(([n])=>n==='courseNo')).toHaveLength(1);
      expect(Object.keys(members)).toHaveLength(FA_FORM_MEMBERS.length+2);
      expect(h.lms.state.saved.get(String(FIRST_TESTNO))).toEqual({testname:'Invented DA 1'});
      expect(steps(h)).toEqual(FULL_STEPS);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r8_a_fa_save_of_a_source_paper_stages_its_copied_items',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger);
      const {out}=await h.saved(faPaperInput());
      expect(out).toMatchObject({status:'saved',academyWrites:4});
      expect(h.lms.requests.find(r=>r.route.includes('SaveMunhang'))!.query.get('exam_list')).toBe('100000001|_|1|,|100000002|_|1|,|100000003|_|2|,|100000004|_|3');
      expect(h.lms.state.saved.get(String(FIRST_TESTNO))).toEqual({testname:'Invented FA 2'});
    }finally{await s.cleanup();}
  });
  test('exam_writes_r8_a_page_that_differs_from_the_pin_is_refused_at_step_a_after_only_the_allocating_get',async()=>{
    const s=await scratch();
    try{
      const drifts:[string,(l:FixtureExamLms)=>void,string][]=[
        ['a changed function',l=>{l.state.pages.fa.creation=t=>creationPage('fa',t,'function other(a) { return 2; }').replace('var v = 2;','var v = 9;');},'wire_hash_mismatch'],
        ['a missing function',l=>{l.state.pages.fa.creation=t=>creationPage('fa',t).replace('onSaveButtonClick','somethingElse');},'wire_hash_mismatch'],
        ['a missing form member',l=>{l.state.pages.fa.creation=t=>creationPage('fa',t,'',{omit:['objectviews']});},'wire_hash_mismatch'],
        ['no paper number',l=>{l.state.pages.fa.creation=t=>creationPage('fa',t).replace(`value="${t}"`,'value=""');},'wire_hash_mismatch'],
        ['no form1',l=>{l.state.pages.fa.creation=t=>creationPage('fa',t).replace('<form name="form1"','<form name="form2"');},'wire_hash_mismatch'],
        ['testgubun not an option',l=>{l.state.pages.fa.creation=t=>creationPage('fa',t).replace('<option value="1180">a</option>','');},'option_not_on_page'],
        ['no usable makecollege',l=>{l.state.pages.fa.creation=t=>creationPage('fa',t).replace('<option value="501">Invented college</option>','');},'make_college_missing'],
      ];
      let n=0;
      for(const [label,setup,code] of drifts){
        const h=harness(join(s.dir,`drift-${n++}.jsonl`));setup(h.lms);
        const review=await h.reviewed(faInput());
        h.lms.requests.length=0;
        const out:any=await h.service.save(h.approve(review));
        expect({label,out}).toMatchObject({label,out:{status:'refused',code,academyWrites:1,effectStatus:'absent',parentSends:0,automaticRetryAllowed:false}});
        // Exactly one write-class request (the allocating GET), no POST, nothing staged; the attempt is resolved absent.
        expect({label,routes:h.lms.routes()}).toEqual({label,routes:[`GET ${FA}TestPageRegServlet Auto [write]`]});
        expect(h.lms.requests.some(r=>r.method==='POST')).toBe(false);
        expect(h.lms.state.staged.size).toBe(0);
        expect(steps(h).at(-1)).toBe('A:reconciled:absent');
        expect(last(h).state).toBe('absent');
        // An absent attempt blocks nothing: the same students can be reviewed and saved again at once.
        h.lms.state.pages.fa.creation=null;
        expect(await h.service.review(faInput())).toMatchObject({status:'reviewed'});
      }
      // da: the course is not an option of the page.
      const h=harness(join(s.dir,'drift-da.jsonl'));h.lms.state.pages.da.creation=t=>creationPage('da',t).replace(`<option value="${COURSE_NO}">Invented course</option>`,'');
      const review=await h.reviewed(daInput());h.lms.requests.length=0;
      expect(await h.service.save(h.approve(review))).toMatchObject({code:'option_not_on_page',academyWrites:1,effectStatus:'absent'});
      expect(h.lms.writes()).toHaveLength(1);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r8_the_assignment_page_must_match_its_pin_and_a_paper_with_assignment_holders_is_refused',async()=>{
    const s=await scratch();
    try{
      const run=async(setup:(l:FixtureExamLms)=>void,code:string,extra:Record<string,unknown>={})=>{
        const h=harness(join(s.dir,`c-${code}-${Math.random()}.jsonl`));setup(h.lms);
        const review=await h.reviewed(faInput());h.lms.requests.length=0;
        const out:any=await h.service.save(h.approve(review));
        expect({code,out}).toMatchObject({code,out:{status:'refused',code,...extra}});
        return {h,out};
      };
      // The assignment page drifted: A and B are read back, nothing of step C is written.
      let r=await run(l=>{l.state.pages.fa.assignment=assignmentPage('function extra(a) { return 1; }').replace('var v = 1;','var v = 8;');},'wire_hash_mismatch',
        {academyWrites:3,effectStatus:'partially_persisted',testno:String(FIRST_TESTNO)});
      expect(r.h.lms.writes()).toHaveLength(3);expect(last(r.h).state).toBe('open');
      // Some student already holds this paper.
      r=await run(l=>{l.state.validateAnswer=()=>'<root><result>Some Name</result><info>Invented earlier test</info></root>';},'assign_duplicate',{academyWrites:3,effectStatus:'partially_persisted'});
      expect(r.h.lms.writes()).toHaveLength(3);expect(JSON.stringify(r.out)).not.toContain('Some Name');
      for(const body of ['not xml','<root><oops/></root>','<a><b></a>','']){
        r=await run(l=>{l.state.validateAnswer=()=>body;},'assign_validate_unreadable',{academyWrites:3,effectStatus:'partially_persisted'});
        expect(r.h.lms.writes()).toHaveLength(3);
      }
    }finally{await s.cleanup();}
  });
  test('exam_writes_r14_a_failed_or_unconfirmed_step_leaves_the_attempt_open_and_sends_no_further_write',async()=>{
    const s=await scratch();
    try{
      const run=async(setup:(l:FixtureExamLms)=>void,expected:Record<string,unknown>,writes:number,lastStep:string)=>{
        const h=harness(join(s.dir,`u-${Math.random()}.jsonl`));setup(h.lms);
        const review=await h.reviewed(faInput());h.lms.requests.length=0;
        const out:any=await h.service.save(h.approve(review));
        expect(out).toMatchObject({parentSends:0,automaticRetryAllowed:false,...expected});
        expect(h.lms.writes()).toHaveLength(writes);
        expect(steps(h).at(-1)).toBe(lastStep);expect(last(h).state).toBe('open');
        return {h,out};
      };
      // Step A: result_sqlerr, an unreadable answer, a staged list whose length is not total_cnt.
      await run(l=>{l.state.saveMunhangAnswer=()=>'<root><result>result_sqlerr</result></root>';},{status:'refused',code:'stage_conflict',academyWrites:null,effectStatus:'unknown'},2,'A:write_dispatched:open');
      await run(l=>{l.state.saveMunhangAnswer=()=>'garbage';},{status:'unverified',code:'exam_response_unverified',academyWrites:null,effectStatus:'unknown'},2,'A:write_dispatched:open');
      await run(l=>{l.state.saveMunhangAnswer=()=>'<root><result>result_ok</result></root>';},{status:'unverified',code:'exam_response_unverified'},2,'A:write_dispatched:open');
      await run(l=>{l.state.staged.set(String(FIRST_TESTNO),[{sel_type:'obj',outer_lecture_key:1,lecture_key:1}]);
        l.state.saveMunhangAnswer=()=>'<root><result>result_ok</result><start_bunit_no>1</start_bunit_no><start_munit_no>1</start_munit_no><end_bunit_no>1</end_bunit_no><end_munit_no>1</end_munit_no><total_cnt>9</total_cnt><total_object>9</total_object><total_subject>0</total_subject><total_descript>0</total_descript><book_no>1</book_no><subject_no>1</subject_no><grade_no>1</grade_no></root>';},
        {status:'unverified',code:'exam_readback_unverified',academyWrites:null,effectStatus:'unknown'},2,'A:response_received:open');
      // Step B: CheckMunhang refuses (a count of -3 is check_munhang_3: 0 writes of step B), the paper POST answers UPLOAD_FAIL or nothing readable, the readback does not show the paper.
      let r=await run(l=>{l.state.checkCount=-3;},{status:'refused',code:'check_munhang_3',academyWrites:2,effectStatus:'partially_persisted',testno:String(FIRST_TESTNO)},2,'A:response_received:open');
      expect(r.h.lms.requests.some(q=>q.method==='POST'&&q.route.includes('Save'))).toBe(false);
      for(const [count,code] of [[0,'check_munhang_0'],[-7,'check_munhang_7'],[-8,'check_munhang_unreadable']] as const)
        await run(l=>{l.state.checkCount=count;},{status:'refused',code,academyWrites:2,effectStatus:'partially_persisted'},2,'A:response_received:open');
      await run(l=>{l.state.paperSaveSaves=false;},{status:'refused',code:'save_failed',academyWrites:null,effectStatus:'unknown'},3,'B:write_dispatched:open');
      await run(l=>{l.state.paperSaveAnswer=()=>'<html>nothing</html>';},{status:'unverified',code:'exam_response_unverified'},3,'B:write_dispatched:open');
      r=await run(l=>{l.state.autoModifyAnswer=()=>'<html>no form</html>';},{status:'unverified',code:'exam_readback_unverified',academyWrites:null,effectStatus:'unknown'},3,'B:response_received:open');
      expect(r.h.lms.requests.some(q=>q.route.includes('TestHangServlet'))).toBe(false);          // an unconfirmed paper is not assigned
      await run(l=>{l.state.autoModifyAnswer=()=>'<html><form name="form1"><input name="p_original_testno" value="1"><input name="testname" value="Another"></form></html>';},{code:'exam_readback_unverified'},3,'B:response_received:open');
      // Step C: the assignment answers something other than success, or the readback does not show every target.
      await run(l=>{l.state.hangSaveAnswer=()=>'<root><result>failed</result></root>';},{status:'refused',code:'assign_failed',academyWrites:null,effectStatus:'unknown'},4,'C:write_dispatched:open');
      await run(l=>{l.state.hangSaveAnswer=()=>'not xml';},{code:'assign_failed'},4,'C:write_dispatched:open');
      await run(l=>{l.state.hangSaveAssigns=false;},{status:'unverified',code:'exam_readback_unverified',academyWrites:null,effectStatus:'unknown'},4,'C:response_received:open');
      // only one of the two targets holds the paper afterwards
      await run(l=>{l.state.onRequest=q=>{if(q.route.includes('TestHangServlet Save'))l.state.assigned.set(String(FIRST_TESTNO),[NAME_A]),l.state.hangSaveAssigns=false;};},{status:'unverified',code:'exam_readback_unverified'},4,'C:response_received:open');
    }finally{await s.cleanup();}
  });
  test('exam_writes_r10_a_request_that_fails_after_a_write_leaves_the_attempt_open_and_a_dropped_write_is_rethrown',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      crashAt(h.lms,'getTestExamList');                                                         // the readback of step A fails to answer
      const out:any=await h.service.save(h.approve(review));
      expect(out).toMatchObject({status:'unverified',code:'exam_readback_unverified',effectStatus:'unknown',academyWrites:null});
      const h2=harness(join(s.dir,'two.jsonl')),review2=await h2.reviewed(faInput());
      crashAt(h2.lms,'SaveMunhang');
      await expect(h2.service.save(h2.approve(review2))).rejects.toThrow('invented_connection_dropped');
      expect(steps(h2).at(-1)).toBe('A:write_dispatched:open');
      expect(existsSync(join(s.dir,'two.jsonl.lock'))).toBe(false);
      const h3=harness(join(s.dir,'three.jsonl')),review3=await h3.reviewed(faInput());
      crashAt(h3.lms,'Auto');
      await expect(h3.service.save(h3.approve(review3))).rejects.toThrow('invented_connection_dropped');
      expect(steps(h3)).toEqual(['A:before_write:open','A:write_dispatched:open']);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r10_a_record_that_cannot_be_made_before_a_write_refuses_ledger_unavailable_and_sends_no_write',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.requests.length=0;
      // The ledger becomes unreadable after the token check: the first record of the attempt cannot be made.
      const append=h.service.ledger.append.bind(h.service.ledger);
      (h.service.ledger as any).append=(r:any)=>{if(r.kind==='attempt')throw Object.assign(Error('ledger_unavailable'),{code:'ledger_unavailable'});return append(r);};
      expect(await h.service.save(h.approve(review))).toMatchObject({status:'refused',code:'ledger_unavailable'});
      expect(h.lms.writes()).toEqual([]);
    }finally{await s.cleanup();}
  });
});

// ---- R11 to R13: locks, open attempts and the reconciling review -----------------------------------------
describe('exam writes R11 to R13: open attempts, reconciliation and the step C review',()=>{
  const openAfterStageConflict=async(h:Harness,input:Record<string,unknown>=faInput())=>{
    h.lms.state.saveMunhangAnswer=()=>'<root><result>result_sqlerr</result></root>';
    const review=await h.reviewed(input),out=await h.service.save(h.approve(review));
    h.lms.state.saveMunhangAnswer=null;
    expect(out).toMatchObject({code:'stage_conflict'});
    h.lms.requests.length=0;
    return review;
  };
  test('exam_writes_r11_an_open_attempt_of_the_same_family_with_an_intersecting_target_blocks_a_save_with_0_requests',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger);
      const other=await h.reviewed(faInput({testName:'Invented FA 5'}));                  // reviewed before the attempt opens
      await openAfterStageConflict(h);
      expect(await h.service.save(h.approve(other))).toMatchObject({status:'refused',code:'reconcile_blocks_retry',academyWrites:0});
      expect(h.lms.requests).toEqual([]);
      // A review that names exactly the attempt's students reconciles it, and the others are not blocked by it.
      const h2=harness(s.ledger,{lms:h.lms});
      h.lms.state.students.push({student_name:'Invented Gamma',mem_id:'invented-login-c',cls_name:'Invented Class C',grade_no:2,cls_no:33,pri_no:77003});
      const third=await h2.reviewed(faInput({targets:[{studentName:'Invented Gamma',studentKey:'77003'}],testName:'Invented FA 6'}));
      expect(await h2.service.save(h2.approve(third))).toMatchObject({status:'saved'});
      // The other family is not blocked either: an open attempt of fa does not hold the same students' da exam.
      const da=await h2.reviewed(daInput({testName:'Invented DA 5'}));
      expect(await h2.service.save(h2.approve(da))).toMatchObject({status:'saved',family:'da'});
    }finally{await s.cleanup();}
  });
  test('exam_writes_r11_a_live_lock_refuses_attempt_already_open_with_0_requests',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.requests.length=0;
      await writeFile(s.ledger+'.lock',lockBody(process.pid),{mode:0o600});
      expect(await h.service.save(h.approve(review))).toMatchObject({status:'refused',code:'attempt_already_open',academyWrites:0});
      expect(await h.service.review(faInput())).toMatchObject({status:'refused',code:'attempt_already_open'});
      expect(h.lms.requests).toEqual([]);
      await rm(s.ledger+'.lock');
      expect(await h.service.save(h.approve(review))).toMatchObject({status:'saved'});
    }finally{await s.cleanup();}
  });
  test('exam_writes_r11_a_stale_lock_with_an_overlapping_unresolved_attempt_blocks_and_a_clean_one_proceeds',async()=>{
    const s=await scratch();
    try{
      const dead=Bun.spawn(['true']);await dead.exited;
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.requests.length=0;
      await writeFile(s.ledger+'.lock',lockBody(dead.pid),{mode:0o600});
      expect(await h.service.save(h.approve(review))).toMatchObject({status:'saved'});         // no unresolved attempt: cleaned, goes on
      const h2=harness(join(s.dir,'two.jsonl')),review2=await h2.reviewed(faInput());
      const review3=await h2.reviewed(faInput({testName:'Invented FA 4'}));
      h2.lms.state.saveMunhangAnswer=()=>'<root><result>result_sqlerr</result></root>';
      await h2.service.save(h2.approve(review2));h2.lms.requests.length=0;
      await writeFile(join(s.dir,'two.jsonl.lock'),lockBody(dead.pid,'crashed-run'),{mode:0o600});
      expect(await h2.service.save(h2.approve(review3))).toMatchObject({status:'refused',code:'reconcile_blocks_retry',academyWrites:0});
      expect(existsSync(join(s.dir,'two.jsonl.lock'))).toBe(false);
      expect(h2.lms.requests).toEqual([]);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r12_a_review_that_names_other_students_than_the_open_attempt_lists_its_keys_with_0_requests',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger);
      await openAfterStageConflict(h);
      const out:any=await h.service.review(faInput({targets:[TARGETS[0]!]}));
      expect(out).toMatchObject({status:'refused',code:'reconcile_blocks_retry',academyWrites:0,studentKeys:[KEY_A,KEY_B]});
      expect(h.lms.requests).toEqual([]);
      expect(JSON.stringify(out)).not.toContain(NAME_A);
      // The same students in a different order reconcile (the attempt left no staged item here, so it is absent).
      const again:any=await h.service.review(faInput({targets:[...TARGETS].reverse()}));
      expect(again).toMatchObject({status:'reviewed'});
    }finally{await s.cleanup();}
  });
  test('exam_writes_r12_r13_an_attempt_with_no_recorded_testno_or_no_staged_item_is_absent_and_the_review_answers_a_fresh_token',async()=>{
    const s=await scratch();
    try{
      // A crash before the allocating GET answered: no testno was recorded.
      const h=harness(join(s.dir,'one.jsonl'));
      const review=await h.reviewed(faInput());crashAt(h.lms,'Auto');
      await expect(h.service.save(h.approve(review))).rejects.toThrow();
      h.lms.requests.length=0;
      h.clock.t+=1000;                                                                              // a new review is a new expiry, so a new token
      const fresh:any=await h.service.review(faInput());
      expect(fresh).toMatchObject({status:'reviewed',change:{steps:'ABC'}});
      expect(fresh.reviewToken).not.toBe(review.reviewToken);
      expect(last(h)).toMatchObject({step:'reconciled',state:'absent'});
      expect(h.lms.routes().filter(r=>!r.includes('ObjectSelect'))).toEqual([]);                    // no readback: there is no testno
      expect(await h.service.save(h.approve(fresh))).toMatchObject({status:'saved'});
      // A crash at SaveMunhang: a testno is recorded and no item is staged (paper_exam_list_empty).
      const h2=harness(join(s.dir,'two.jsonl')),review2=await h2.reviewed(faInput());crashAt(h2.lms,'SaveMunhang');
      await expect(h2.service.save(h2.approve(review2))).rejects.toThrow();
      expect(last(h2).testno).toBe(String(FIRST_TESTNO));
      h2.lms.requests.length=0;
      expect(await h2.service.review(faInput())).toMatchObject({status:'reviewed',change:{steps:'ABC'}});
      expect(h2.lms.routes().filter(r=>!r.includes('ObjectSelect'))).toEqual(['POST /servlet/controller.common.TestpageSelectExServlet getTestExamList']);
      expect(last(h2)).toMatchObject({step:'reconciled',state:'absent',testno:String(FIRST_TESTNO)});
      // A failed read is not an absent attempt: paper_exam_list_unverified, a transport error.
      const h3=harness(join(s.dir,'three.jsonl')),review3=await h3.reviewed(faInput());crashAt(h3.lms,'SaveMunhang');
      await expect(h3.service.save(h3.approve(review3))).rejects.toThrow();
      h3.lms.state.onRequest=r=>{if(r.route.includes('getTestExamList'))throw Error('invented_transport_failure');};
      expect(await h3.service.review(faInput())).toMatchObject({status:'refused',code:'reconcile_blocks_retry',studentKeys:[KEY_A,KEY_B]});
      expect(last(h3).state).toBe('open');
    }finally{await s.cleanup();}
  });
  test('exam_writes_r12_items_staged_and_the_paper_not_saved_is_partially_persisted_with_no_continuation',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.state.checkCount=-2;
      expect(await h.service.save(h.approve(review))).toMatchObject({code:'check_munhang_2',effectStatus:'partially_persisted',testno:String(FIRST_TESTNO)});
      expect(h.lms.state.staged.get(String(FIRST_TESTNO))).toHaveLength(4);
      // The next save of overlapping students is blocked until a review reconciles.
      const next=await h.service.save(h.approve(review));
      expect(next).toMatchObject({status:'refused',code:'review_token_invalid'});                      // the token was used
      h.lms.state.checkCount=1;h.lms.requests.length=0;
      const answer:any=await h.service.review(faInput());
      expect(answer).toMatchObject({status:'partially_persisted',done:false,nothingToSave:true,testno:String(FIRST_TESTNO),reconciled:'items_staged_paper_not_saved',effectStatus:'partially_persisted',academyWrites:0});
      expect(answer.reviewToken).toBeUndefined();
      expect(answer.nextStep).toContain('stays unused');
      expect(h.lms.writes()).toEqual([]);
      expect(last(h)).toMatchObject({step:'reconciled',state:'partially_persisted',testno:String(FIRST_TESTNO)});
      // A new review and save create a new paper from step A.
      h.clock.t+=1000;
      const {out}=await h.saved(faInput());
      expect(out).toMatchObject({status:'saved',testno:String(FIRST_TESTNO+1),academyWrites:4});
      expect(h.lms.state.allocated).toEqual([FIRST_TESTNO,FIRST_TESTNO+1]);
    }finally{await s.cleanup();}
  });
  test('exam_writes_r13_a_saved_paper_without_assignment_is_answered_with_a_review_of_step_c_alone_that_never_creates_a_second_paper',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      crashAt(h.lms,'TestHangServlet Save');
      await expect(h.service.save(h.approve(review))).rejects.toThrow('invented_connection_dropped');
      expect(h.lms.state.saved.has(String(FIRST_TESTNO))).toBe(true);
      expect(h.lms.state.assigned.size).toBe(0);
      h.lms.requests.length=0;
      const c:any=await h.service.review(faInput());
      expect(c).toMatchObject({status:'reviewed',reconciled:'paper_saved_not_assigned',testno:String(FIRST_TESTNO),
        change:{steps:'C',family:'fa',testName:'Invented FA 1',testno:String(FIRST_TESTNO),munhangCount:4,...PERIOD}});
      expect(c.change.items).toBeUndefined();expect(c.change.testgubunNo).toBeUndefined();
      expect(c.preview).toMatchObject({steps:'C',testno:String(FIRST_TESTNO),munhangCount:4});
      expect(h.lms.routes().filter(r=>!r.includes('ObjectSelect'))).toEqual(['POST /servlet/controller.common.TestpageSelectExServlet getTestExamList',`GET ${FA}TestPageRegServlet AutoModify`,`GET ${FA}TestHangServlet Validate`]);
      expect(h.lms.writes()).toEqual([]);
      expect(last(h)).toMatchObject({step:'reconciled',state:'partially_persisted',examStep:'C'});
      expect(c.reviewToken).toBe(reviewTokenOf({tool:EXAM_TOOL_FAMILY,change:c.change,targets:TARGET_STRINGS,scopeKey:review.scopeKey,wireSha256:review.wireSha256,expiresAt:c.expiresAt}));
      // Its save sends step C's requests only: two pre-write reads, the assignment GET, the readback.
      h.lms.requests.length=0;
      const out:any=await h.service.save(h.approve(c));
      expect(out).toMatchObject({status:'saved',steps:'C',testno:String(FIRST_TESTNO),academyWrites:1,effectStatus:'persisted'});
      expect(h.lms.routes()).toEqual([`GET ${FA}TestHangServlet Main`,`GET ${FA}TestHangServlet Validate`,`GET ${FA}TestHangServlet Save [write]`,`GET ${FA}TestHangServlet Validate`]);
      expect(h.lms.requests.find(r=>r.route.includes('Save'))!.query.get('munhangCount')).toBe('4');
      expect(steps(h).slice(-4)).toEqual(['C:before_write:open','C:write_dispatched:open','C:response_received:open','C:reconciled:persisted']);
      expect(h.lms.state.allocated).toEqual([FIRST_TESTNO]);
      expect(h.lms.state.assigned.get(String(FIRST_TESTNO))).toEqual([NAME_A,NAME_B]);
      // A token is used once.
      expect(await h.service.save(h.approve(c))).toMatchObject({code:'review_token_invalid'});
    }finally{await s.cleanup();}
  });
  test('exam_writes_r13_a_step_c_save_that_refuses_before_its_write_leaves_the_attempt_open_for_a_review',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      crashAt(h.lms,'TestHangServlet Save');
      await expect(h.service.save(h.approve(review))).rejects.toThrow();
      const c:any=await h.service.review(faInput());
      h.lms.state.validateAnswer=()=>'<root><result>Some Name</result></root>';
      h.lms.requests.length=0;
      expect(await h.service.save(h.approve(c))).toMatchObject({status:'refused',code:'assign_duplicate',academyWrites:0,effectStatus:'none'});
      expect(h.lms.writes()).toEqual([]);
      expect(last(h).state).toBe('open');
      // The next review still sees the saved paper and offers step C again, not a second paper.
      h.lms.state.validateAnswer=null;
      expect(await h.service.review(faInput())).toMatchObject({status:'reviewed',change:{steps:'C',testno:String(FIRST_TESTNO)}});
    }finally{await s.cleanup();}
  });
  test('exam_writes_r12_r13_an_assignment_read_back_for_every_target_is_persisted_and_offers_nothing_to_save',async()=>{
    const s=await scratch();
    try{
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      h.lms.state.hangSaveAssigns=true;
      h.lms.state.onRequest=r=>{if(r.route.includes('TestHangServlet Save'))h.lms.state.onRequest=q=>{if(q.route.includes('Validate'))h.lms.state.validateAnswer=()=>'garbage';};};
      const out:any=await h.service.save(h.approve(review));
      expect(out).toMatchObject({status:'unverified',code:'exam_readback_unverified',effectStatus:'unknown',academyWrites:null});
      h.lms.state.onRequest=null;h.lms.state.validateAnswer=null;h.lms.requests.length=0;
      const done:any=await h.service.review(faInput());
      expect(done).toMatchObject({status:'persisted',done:true,nothingToSave:true,effectStatus:'persisted',academyWrites:0});
      expect(done.reviewToken).toBeUndefined();
      expect(last(h)).toMatchObject({step:'reconciled',state:'persisted'});
      expect(h.lms.writes()).toEqual([]);
      // Nothing is blocked afterwards: a new exam for the same students starts at step A.
      expect(await h.service.review(faInput({testName:'Invented FA 4'}))).toMatchObject({status:'reviewed',change:{steps:'ABC'}});
    }finally{await s.cleanup();}
  });
  test('exam_writes_r12_any_other_readback_leaves_the_attempt_open_and_the_review_refuses_reconcile_blocks_retry',async()=>{
    const s=await scratch();
    try{
      const mk=async(name:string)=>{
        const h=harness(join(s.dir,name+'.jsonl')),review=await h.reviewed(faInput());
        crashAt(h.lms,'TestHangServlet Save');
        await expect(h.service.save(h.approve(review))).rejects.toThrow();
        return h;
      };
      const blocked=async(h:Harness,label:string)=>{
        h.lms.requests.length=0;
        expect({label,out:await h.service.review(faInput())}).toMatchObject({label,out:{status:'refused',code:'reconcile_blocks_retry',academyWrites:0,studentKeys:[KEY_A,KEY_B]}});
        expect(h.lms.writes()).toEqual([]);expect(last(h).state).toBe('open');
      };
      let h=await mk('partial');                                    // an assignment for only some targets
      h.lms.state.assigned.set(String(FIRST_TESTNO),[NAME_A]);await blocked(h,'only some targets');
      h=await mk('count');                                          // a staged count other than total_cnt
      h.lms.state.staged.set(String(FIRST_TESTNO),h.lms.state.staged.get(String(FIRST_TESTNO))!.slice(1));await blocked(h,'staged count');
      h=await mk('modify');                                         // the readback of the saved paper fails
      h.lms.state.autoModifyAnswer=()=>{throw Error('invented_transport_failure');};await blocked(h,'AutoModify failed');
      h=await mk('name');                                           // a paper of that number with another name
      h.lms.state.saved.set(String(FIRST_TESTNO),{testname:'Another name'});await blocked(h,'another name');
      h=await mk('validate');                                       // the assignment readback is unreadable
      h.lms.state.validateAnswer=()=>'garbage';await blocked(h,'Validate unreadable');
    }finally{await s.cleanup();}
  });
});

// ---- R16 and the ledger shape -----------------------------------------------------------------------------
describe('exam writes R16 and the ledger',()=>{
  test('exam_writes_r16_no_save_answer_error_or_record_holds_a_name_cookie_or_password',async()=>{
    const s=await scratch();
    try{
      const answers:unknown[]=[];
      const h=harness(s.ledger),review=await h.reviewed(faInput());
      answers.push(await h.service.save(h.approve(review,{userApproved:false})),await h.service.save(h.approve(review,{reviewToken:hex('a')})),await h.service.save(h.approve(review)));
      const scenarios:[string,(l:FixtureExamLms)=>void][]=[
        ['check',l=>{l.state.checkCount=-1;}],
        ['duplicate',l=>{l.state.validateAnswer=()=>`<root><result>${NAME_A}, ${NAME_B}</result></root>`;}],
        ['stage',l=>{l.state.saveMunhangAnswer=()=>'<root><result>result_sqlerr</result></root>';}],
        ['upload',l=>{l.state.paperSaveSaves=false;}],
      ];
      for(const [name,setup] of scenarios){
        const h2=harness(join(s.dir,`r16-${name}.jsonl`));setup(h2.lms);
        answers.push((await h2.saved(faInput())).out,await h2.service.review(faInput({targets:[TARGETS[0]!]})));      // the refused review lists keys only
        for(const secret of [NAME_A,NAME_B,'invented-login',COOKIE_CANARY,'Invented FA 1'])expect(readFileSync(h2.service.ledger.path,'utf8')).not.toContain(secret);
      }
      for(const answer of answers)for(const secret of [NAME_A,NAME_B,'Invented FA 1','invented-login',COOKIE_CANARY])expect(JSON.stringify(answer)).not.toContain(secret);
      for(const secret of [NAME_A,NAME_B,'Invented FA 1',COOKIE_CANARY])expect(readFileSync(s.ledger,'utf8')).not.toContain(secret);
      expect(readdirSync(s.dir).filter(f=>f.endsWith('.lock'))).toEqual([]);
    }finally{await s.cleanup();}
  });
  test('exam_writes_the_ledger_validates_exam_records_by_their_own_rule_and_clinic_records_by_theirs',async()=>{
    const s=await scratch();
    try{
      const ledger=new GatedWriteLedger(s.ledger);
      const attempt={kind:'attempt',attemptId:'a',tool:EXAM_TOOL_FAMILY,reviewToken:hex('a'),changeSha256:hex('b'),targets:TARGET_STRINGS,scopeKey:'fa:'+hex('c'),step:'before_write',examStep:'A',state:'open',
        at:new Date(T0).toISOString(),runId:'r'} as const;
      expect(()=>ledger.append(attempt as any)).not.toThrow();
      expect(()=>ledger.append({...attempt,testno:'424001',step:'response_received'} as any)).not.toThrow();
      for(const bad of [{studentKey:KEY_A},{targets:undefined},{targets:[]},{targets:[...TARGET_STRINGS].reverse()},{targets:[TARGET_STRINGS[0],TARGET_STRINGS[0]]},{targets:['1!77001!2']},
        {examStep:undefined},{examStep:'D'},{testno:'x1'},{testno:'0'},{targets:Array.from({length:41},(_,i)=>`1!${80000+i}!2!3`)}])
        expect({bad,out:(()=>{try{ledger.append({...attempt,...bad} as any);return 'appended';}catch(e:any){return e.message;}})()}).toEqual({bad,out:'ledger_unavailable'});
      const clinic={kind:'attempt',attemptId:'a',tool:CLINIC_TOOL_FAMILY,reviewToken:hex('a'),changeSha256:hex('b'),studentKey:KEY_A,scopeKey:'clinic:88001',step:'before_write',state:'open',at:new Date(T0).toISOString(),runId:'r'};
      expect(()=>ledger.append(clinic as any)).not.toThrow();
      for(const bad of [{studentKey:undefined},{targets:TARGET_STRINGS},{examStep:'A'},{testno:'424001'}])
        expect({bad,out:(()=>{try{ledger.append({...clinic,...bad} as any);return 'appended';}catch(e:any){return e.message;}})()}).toEqual({bad,out:'ledger_unavailable'});
      // The reviewed record of an exam has targets and no student; a clinic one keeps its student.
      const reviewed={kind:'reviewed',tool:EXAM_TOOL_FAMILY,reviewToken:hex('a'),changeSha256:hex('b'),targets:TARGET_STRINGS,scopeKey:'fa:'+hex('c'),wireSha256:hex('d'),reviewedAt:'2040-01-16T09:00:00.000Z',expiresAt:'2040-01-16T09:15:00.000Z'};
      expect(()=>ledger.append(reviewed as any)).not.toThrow();
      expect(()=>ledger.append({...reviewed,studentKey:KEY_A} as any)).toThrow('ledger_unavailable');
      expect(()=>ledger.append({...reviewed,tool:CLINIC_TOOL_FAMILY} as any)).toThrow('ledger_unavailable');
      expect(ledger.read()).toHaveLength(4);
    }finally{await s.cleanup();}
  });
});

// ---- the parsers -------------------------------------------------------------------------------------------
describe('exam writes: the answer parsers',()=>{
  test('exam_writes_parse_the_xml_answers_of_the_three_steps',()=>{
    expect(parseSaveMunhang('<?xml version="1.0"?><r><result>result_sqlerr</result></r>')).toEqual({kind:'conflict'});
    expect(parseSaveMunhang('<r><result>other</result></r>')).toEqual({kind:'unreadable'});
    expect(parseSaveMunhang('<r><result>result_ok</result><total_cnt>0</total_cnt></r>')).toEqual({kind:'unreadable'});
    expect(parseSaveMunhang('<r><result>result_ok</result><start_bunit_no>1</start_bunit_no><start_munit_no>2</start_munit_no><end_bunit_no>3</end_bunit_no><end_munit_no>4</end_munit_no><total_cnt>5</total_cnt><total_object>3</total_object><total_subject>1</total_subject><total_descript>1</total_descript><book_no>6</book_no><subject_no>7</subject_no><grade_no>8</grade_no></r>'))
      .toEqual({kind:'ok',staged:{startBunitNo:'1',startMunitNo:'2',endBunitNo:'3',endMunitNo:'4',totalCnt:5,totalObject:'3',totalSubject:'1',totalDescript:'1',bookNo:'6',subjectNo:'7',gradeNo:'8'}});
    expect(parseCheckMunhang('<r><count>3</count><qno>9</qno></r>')).toEqual({kind:'ok'});
    for(const [body,code] of [['<r><count>0</count></r>','check_munhang_0'],['<r><count>-1</count></r>','check_munhang_1'],['<r><count>-7</count></r>','check_munhang_7'],['<r><count>-9</count></r>','check_munhang_unreadable'],
      ['<r><qno>1</qno></r>','check_munhang_unreadable'],['nope','check_munhang_unreadable'],['<r><count>x</count></r>','check_munhang_unreadable']])
      expect(parseCheckMunhang(body)).toEqual({kind:'refused',code});
    expect(parseSaveLogic('<script>var logic = "UPLOAD_OK";</script>')).toBe('UPLOAD_OK');
    expect(parseSaveLogic('logic=UPLOAD_FAIL')).toBe('UPLOAD_FAIL');
    expect(parseSaveLogic('nothing')).toBeNull();expect(parseSaveLogic('UPLOAD_OK UPLOAD_FAIL')).toBeNull();
    expect(parseValidate('<r><result>success</result></r>')).toEqual({kind:'success'});
    expect(parseValidate('<r><result>A, B</result><info>x</info></r>')).toEqual({kind:'names',text:'A, B'});
    expect(parseValidate('<r><info>x</info></r>')).toEqual({kind:'unreadable'});expect(parseValidate('<r><result>x</r>')).toEqual({kind:'unreadable'});
    expect(parseCoursePaperNo('{"pNo":6100,"result":"OK"}')).toBe('6100');expect(parseCoursePaperNo('{"pNo":"6100"}')).toBe('6100');
    expect([...xmlValues('<a><b> x &amp; y </b><c/><d><![CDATA[<z>]]></d></a>')]).toEqual([['b','x & y'],['c',''],['d','<z>']]);
    for(const bad of ['','<a>','<a></b>','<a/><b/>','text','<a><b></a></b>'])expect({bad,out:(()=>{try{xmlValues(bad);return 'parsed';}catch{return 'unparsable';}})()}).toEqual({bad,out:'unparsable'});
  });
  test('exam_writes_the_paper_exam_list_carries_outer_lecture_key_or_null_and_refuses_an_out_of_bound_value',()=>{
    const list=(rows:unknown[])=>JSON.stringify({result:'OK',exam_list:rows});
    expect(parsePaperExamList(list([{sel_type:'obj',lecture_key:5,outer_lecture_key:100000001},{sel_type:'des',lecture_key:6}])).map(r=>r.outerLectureKey)).toEqual([100000001,null]);
    expect(parsePaperExamList(list([{sel_type:'obj',lecture_key:5,outer_lecture_key:'7'}]))[0]!.outerLectureKey).toBe(7);
    expect(()=>parsePaperExamList(list([{sel_type:'obj',lecture_key:5,outer_lecture_key:1000000000}]))).toThrow('paper_exam_list_unverified');
    expect(()=>parsePaperExamList(list([{sel_type:'obj',lecture_key:5,outer_lecture_key:'x'}]))).toThrow('paper_exam_list_unverified');
    expect(parsePaperExamList(list([{sel_type:'obj',lecture_key:5}]))[0]).toMatchObject({lectureKey:5,selType:'obj'});
  });
  test('exam_writes_qualify_exam_change_rebuilds_exactly_the_reviewed_change',()=>{
    const change={steps:'ABC',family:'fa',testName:'T',testgubunNo:1180,items:[{key:'1',type:'obj'}],targets:[{studentName:NAME_A,studentKey:KEY_A,gradeNo:GRADE,clsNo:CLASS_A}],...PERIOD};
    expect(qualifyExamChange(change)).toEqual(change);
    expect(qualifyExamChange({...change,sourcePaperNo:SOURCE_PAPER})).toEqual({...change,sourcePaperNo:SOURCE_PAPER});
    expect(()=>qualifyExamChange({...change,family:'da'})).toThrow('review_token_invalid');
    expect(()=>qualifyExamChange({...change,items:[{key:'1',type:'obj'},{key:'2',type:'x'}]})).toThrow('review_token_invalid');
    const assign={steps:'C',family:'da',testName:'T',testno:'424001',munhangCount:30,targets:change.targets,...PERIOD};
    expect(qualifyExamChange(assign)).toEqual(assign);
    expect(()=>qualifyExamChange({...assign,items:change.items})).toThrow('review_token_invalid');
    expect(()=>qualifyExamChange({...assign,munhangCount:0})).toThrow('review_token_invalid');
    expect(()=>qualifyExamChange({...assign,testno:'0'})).toThrow('review_token_invalid');
  });
});

// ---- the surface, the skills and the bridge's lists ---------------------------------------------------------
describe('exam writes: the surface and the skills',()=>{
  const surface=JSON.parse(readFileSync(resolve(root,'release/surface.json'),'utf8')) as {tools:{name:string;title:string;description:string;inputSchema:any}[]};
  const bytes=(v:unknown)=>Buffer.byteLength(JSON.stringify(v),'utf8');
  test('exam_writes_the_surface_holds_the_two_tools_within_24_tools_and_24000_bytes',()=>{
    const names=surface.tools.map(t=>t.name);
    expect(names.slice(-2)).toEqual(['academy_exam_paper_review','academy_exam_paper_save']);
    expect(surface.tools.length).toBeLessThanOrEqual(24);
    expect(bytes(surface.tools)).toBeLessThanOrEqual(24000);
    const [review,save]=surface.tools.slice(-2) as [any,any];
    expect(bytes(review)+bytes(save)).toBeLessThanOrEqual(1997);
    expect(Object.keys(review.inputSchema.properties)).toEqual(['family','testName','testgubunNo','sourcePaperNo','items','courseNo','targets','start','end','testingMinutes']);
    expect(review.inputSchema.required).toEqual(['family','testName','testgubunNo','targets','start','end','testingMinutes']);
    expect(Object.keys(save.inputSchema.properties)).toEqual(['reviewToken','userApproved','change']);
    expect(save.inputSchema.required).toEqual(['reviewToken','userApproved','change']);
    for(const word of ['fa','da','sourcePaperNo','targets','Read-only'])expect(review.description).toContain(word);
    for(const word of ['approves','userApproved','no retry'])expect(save.description).toContain(word);
  });
  test('exam_writes_every_academy_word_of_the_skills_is_a_surface_tool_and_the_exam_tools_are_named_by_their_pair',()=>{
    const names=new Set(surface.tools.map(t=>t.name));
    const skills=readdirSync(resolve(repo,'content/skills')).filter(d=>d.startsWith('academy-')).map(d=>readFileSync(resolve(repo,'content/skills',d,'SKILL.md'),'utf8'));
    const words=new Set(skills.flatMap(t=>t.match(/\b(?:class2_)?academy_[a-z0-9_]+\b/g)??[]));
    expect([...words].filter(w=>w!=='academy_operations'&&!names.has(w))).toEqual([]);
    expect(words.has('academy_exam_paper_review')&&words.has('academy_exam_paper_save')).toBe(true);
    expect(words.has('academy_exam_paper')).toBe(false);
  });
});
