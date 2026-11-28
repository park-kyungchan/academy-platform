import {randomUUID} from 'node:crypto';
import {z} from 'zod';
import type {McpServer} from '@modelcontextprotocol/server';
import {publicReadFailure} from './read-failure.ts';
import {academyRoutingDescription} from './routing-context.ts';
import {
  GatedWriteLedger,GatedWriteRefusal,REVIEW_TOKEN_TTL_MS,changeSha256Of,openAttempts,reviewTokenOf,sha256Hex,stepsOf,verifyReviewToken,
  type AttemptRecord,type AttemptState,type ExamStep,type LedgerLock,type LedgerRecord,
} from '../../../../../apps/academy/dc-gang-a/src/lms/gatedWriteLedger';
import {
  COURSE_PAPER_PATH,EXAM_TOOL_FAMILY,STUDENT_LOOKUP_PATH,assignMainPath,assignPath,autoModifyPath,autoPath,autoPageTestno,checkMunhangPath,coursePaperForm,examPageSha256,
  examPreview,examScopeKey,examWireSha256,expectedExamPageHash,parseAutoModify,parseCheckMunhang,parseCoursePaperNo,parseCreationPage,parseSaveLogic,parseSaveMunhang,
  parseStudentList,parseValidate,paperSaveMembers,paperSavePath,qualifyExamChange,qualifyExamRequest,qualifyItems,saveMunhangPath,sortedTargetStrings,
  studentKeysOfTargets,studentListForm,validateNamesAll,MAX_ITEMS,
  type ExamAssignChange,type ExamChange,type ExamItem,type ExamPaperChange,type ExamRequest,type ExamTarget,type ExpectedExamWireHashes,type CreationPage,
} from '../../../../../apps/academy/dc-gang-a/src/lms/examPaperWrite';
import type {PaperExamRow} from '../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead';

/**
 * `academy_exam_paper_review` and `academy_exam_paper_save` (specs/academy-mcp/gated-writes.md): the creation of an FA or unit-exam paper and
 * its assignment to named students, in two phases. The review reads (the students, the items, and an unresolved earlier attempt by its readback),
 * freezes one change and answers a `reviewToken`; the save runs the three steps A (stage the items), B (save the paper) and C (assign it) in one
 * call, only with `userApproved: true`, records each step in the private attempt ledger before the request that writes, and reads each step back.
 */
export interface ExamPaperBackend {
  /** A GET of a route classed read (AutoModify, CheckMunhang, TestHangServlet Main and Validate). */
  readExamRoute(path:string):Promise<string>;
  /** A POST form of a route classed read (`getStudentList`, `GetPaperNo`). */
  postExamRead(path:string,fields:Record<string,string>):Promise<string>;
  /** The paper's exam list (a read). */
  readPaperExamList(paperNo:string):Promise<PaperExamRow[]>;
  /** The mutating requests, called only inside the save under the ledger lock: a GET of a route classed write, and the multipart POST of the paper. */
  writeExamGet(path:string):Promise<string>;
  writeExamMultipart(path:string,members:readonly (readonly [string,string])[]):Promise<string>;
}
export interface ExamPaperOptions {
  now?:()=>number;newId?:()=>string;
  /** Test seam: replaces the pinned production page hashes so synthetic pages can exercise every path. */
  expectedWireHashes?:ExpectedExamWireHashes;
  pidAlive?:(pid:number)=>boolean;
}

const optional=z.unknown().optional();
/** The review input. Target and scope members are checked by the service (R7), and unknown members are kept so that a public or class scope is refused by name. */
export const examPaperReviewSchema=z.looseObject({
  family:z.enum(['fa','da']),testName:optional,testgubunNo:optional,sourcePaperNo:optional,items:optional,courseNo:optional,targets:optional,start:optional,end:optional,testingMinutes:optional,
});
/** The save input: exactly the token, the approval and the reviewed change. Their values are checked by the service, in the order of R3 to R7. */
export const examPaperSaveSchema=z.object({reviewToken:z.string().max(200).optional(),userApproved:z.unknown().optional(),change:z.unknown().optional()}).strict();

const CLEAN={parentSends:0,automaticRetryAllowed:false as const};
/** A review refusal that lists the keys of the unresolved attempt it blocks on (keys only, R16). */
class ExamBlocked extends GatedWriteRefusal {constructor(readonly studentKeys:string[]){super('reconcile_blocks_retry');}}
/** An answer that ends the save where it stands: thrown by the step helpers and returned by the call. */
class ExamStop extends Error {constructor(readonly result:Record<string,unknown>){super('exam_stop');}}
const refused=(error:unknown,extra:Record<string,unknown>={})=>{
  const failure=publicReadFailure(error,'exam_source_failed');
  return {status:'refused' as const,code:failure.code,reasonCode:failure.code,failure,academyWrites:0,...(error instanceof ExamBlocked?{studentKeys:error.studentKeys}:{}),...CLEAN,...extra};
};
const numeric=(a:string,b:string)=>Number(a)-Number(b);

/** What a reconciling review reads of one open attempt. */
type Reconciled={state:'absent'}|{state:'persisted'}|{state:'saved_unassigned';testno:string;munhangCount:number}|{state:'staged_only';testno:string};

export class ExamPaperService {
  #tail:Promise<unknown>=Promise.resolve();
  readonly ledger:GatedWriteLedger;
  constructor(readonly backend:ExamPaperBackend|null,ledgerPath:string|null,private readonly options:ExamPaperOptions={}){
    this.ledger=new GatedWriteLedger(ledgerPath,options.pidAlive);
  }
  readonly #now=()=>(this.options.now??Date.now)();
  readonly #id=()=>(this.options.newId??randomUUID)();
  /** One LMS session and one ledger lock serve one call at a time in this process. */
  #serial<T>(run:()=>Promise<T>):Promise<T>{const next=this.#tail.then(run,run);this.#tail=next.catch(()=>undefined);return next;}

  review(raw:unknown){return this.#serial(()=>this.#review(raw));}
  save(raw:unknown){return this.#serial(()=>this.#save(raw));}

  // ---- review ---------------------------------------------------------------------------------------

  /** The items of the request: the teacher's own, the copy of an existing paper, or the 30 items of a DA course paper. */
  async #items(request:ExamRequest):Promise<ExamItem[]>{
    const b=this.backend!,source=request.source;
    if(source.kind==='items')return source.items;
    let rows:PaperExamRow[];
    if(source.kind==='paper'){
      rows=await b.readPaperExamList(source.sourcePaperNo);
      if(rows.length>MAX_ITEMS)throw new GatedWriteRefusal('too_many_items');
    }else{
      const paperNo=parseCoursePaperNo(await b.postExamRead(COURSE_PAPER_PATH,coursePaperForm(source.courseNo,request.testgubunNo)));
      rows=await b.readPaperExamList(paperNo);
      if(rows.length!==MAX_ITEMS)throw new GatedWriteRefusal('da_item_count');
    }
    if(rows.some(r=>r.outerLectureKey===null))throw new GatedWriteRefusal('paper_exam_list_unverified');
    return qualifyItems(rows.map(r=>({key:String(r.outerLectureKey),type:r.selType})));
  }

  /**
   * R12 for an exam attempt, by read-only requests and the recorded `testno`: absent (no testno, or no staged item); persisted (the paper exists and every
   * target holds the assignment); the paper saved and no assignment for any target (R13: a review of step C alone); items staged and the paper not saved
   * (no tool continues it). Anything else, and any failed read, is `null` (the attempt stays open).
   */
  async #classify(attempt:AttemptRecord,records:readonly LedgerRecord[],change:ExamPaperChange,targets:readonly ExamTarget[]):Promise<Reconciled|null>{
    const b=this.backend!,testno=stepsOf(records,attempt.attemptId).map(r=>r.testno).filter((t):t is string=>t!==undefined).at(-1);
    if(testno===undefined)return {state:'absent'};
    let staged:PaperExamRow[];
    try{staged=await b.readPaperExamList(testno);}
    catch(error){return error instanceof Error&&error.message==='paper_exam_list_empty'?{state:'absent'}:null;}
    if(staged.length!==change.items.length)return null;
    let modify:ReturnType<typeof parseAutoModify>;
    try{modify=parseAutoModify(await b.readExamRoute(autoModifyPath(change.family,testno)));}catch{return null;}
    const saved=modify!==null&&modify.testno===testno&&modify.testname===change.testName;
    const notSaved=modify===null||modify.testno!==testno||modify.testname==='';
    if(!saved&&!notSaved)return null;
    if(notSaved)return {state:'staged_only',testno};
    try{
      const assign:ExamAssignChange={steps:'C',family:change.family,testName:change.testName,testno,munhangCount:staged.length,targets:[...targets],start:change.start,end:change.end,testingMinutes:change.testingMinutes};
      const answer=parseValidate(await b.readExamRoute(assignPath(change.family,'Validate',assign,testno,staged.length,this.#now())));
      if(answer.kind==='success')return {state:'saved_unassigned',testno,munhangCount:staged.length};
      if(answer.kind==='names'&&validateNamesAll(answer.text,targets))return {state:'persisted'};
    }catch{/* a failed read leaves the attempt open */}
    return null;
  }

  async #review(raw:unknown){
    try{
      const request=qualifyExamRequest(raw);                           // R7: 0 HTTP requests
      if(!this.backend)return {status:'unavailable' as const,code:'exam_source_port_unavailable',reasonCode:'exam_source_port_unavailable',academyWrites:0,...CLEAN};
      this.ledger.assertAvailable();                                    // R10: before any HTTP request
      const b=this.backend,runId=this.#id();
      const lock=this.ledger.acquireLock(runId,true,this.#now) as LedgerLock;
      try{
        // R11, R12: a review that touches an open attempt of this family must name exactly its students (keys only, 0 requests).
        const keys=request.targets.map(t=>t.studentKey).sort(numeric);
        let records=this.ledger.read();
        const open=openAttempts(records,{tool:EXAM_TOOL_FAMILY,scopePrefix:request.family+':'}).filter(a=>studentKeysOfTargets(a.targets??[]).some(k=>keys.includes(k)));
        const differing=open.filter(a=>studentKeysOfTargets(a.targets??[]).join(',')!==keys.join(','));
        if(differing.length)throw new ExamBlocked([...new Set(differing.flatMap(a=>studentKeysOfTargets(a.targets??[])))].sort(numeric));
        // The targets (reads): exactly one student row each.
        const clsNames=new Map<string,string>(),targets:ExamTarget[]=[];
        for(const t of request.targets){
          const found=parseStudentList(await b.postExamRead(STUDENT_LOOKUP_PATH,studentListForm(t.studentName)),t);
          targets.push(found.target);clsNames.set(t.studentKey,found.clsName);
        }
        targets.sort((x,y)=>sortedTargetStrings([x])[0]!<sortedTargetStrings([y])[0]!?-1:1);
        const targetStrings=sortedTargetStrings(targets),scopeKey=examScopeKey(request.family,targets);
        if(open.some(a=>(a.targets??[]).join('\n')!==targetStrings.join('\n')))throw new ExamBlocked(keys);
        const items=await this.#items(request);
        const paper:ExamPaperChange={steps:'ABC',family:request.family,testName:request.testName,testgubunNo:request.testgubunNo,items,
          ...(request.source.kind==='paper'?{sourcePaperNo:request.source.sourcePaperNo}:{}),...(request.source.kind==='course'?{courseNo:request.source.courseNo}:{}),
          targets,start:request.start,end:request.end,testingMinutes:request.testingMinutes};
        let change:ExamChange=paper,reconciledNote:Record<string,unknown>={};
        if(open.length){
          const outcomes:{attempt:AttemptRecord;found:Reconciled}[]=[];
          for(const a of open){
            const found=await this.#classify(a,records,paper,targets);
            if(found===null)throw new ExamBlocked(keys);
            outcomes.push({attempt:a,found});
          }
          const stateOf=(f:Reconciled):AttemptState=>f.state==='absent'?'absent':f.state==='persisted'?'persisted':'partially_persisted';
          for(const o of outcomes){
            const testno='testno' in o.found?o.found.testno:undefined;
            this.ledger.append({...o.attempt,step:'reconciled',state:stateOf(o.found),at:new Date(this.#now()).toISOString(),runId,...(testno&&!o.attempt.testno?{testno}:{})});
          }
          const done=outcomes.find(o=>o.found.state==='persisted');
          if(done)return {status:'persisted' as const,done:true,nothingToSave:true,family:request.family,scopeKey,attemptId:done.attempt.attemptId,
            reconciled:'persisted_by_read_only_readback',effectStatus:'persisted' as const,academyWrites:0,...CLEAN};
          const staged=outcomes.find(o=>o.found.state==='staged_only')?.found as {testno:string}|undefined;
          if(staged)return {status:'partially_persisted' as const,done:false,nothingToSave:true,family:request.family,scopeKey,testno:staged.testno,
            reconciled:'items_staged_paper_not_saved',effectStatus:'partially_persisted' as const,academyWrites:0,...CLEAN,
            nextStep:'Items were staged under this paper number and the paper was not saved. No tool continues it: the staged number stays unused in the LMS. Run academy_exam_paper_review again to create a new paper from the start.'};
          const unassigned=outcomes.find(o=>o.found.state==='saved_unassigned')?.found as {testno:string;munhangCount:number}|undefined;
          if(unassigned){
            change={steps:'C',family:request.family,testName:request.testName,testno:unassigned.testno,munhangCount:unassigned.munhangCount,targets,start:request.start,end:request.end,testingMinutes:request.testingMinutes};
            reconciledNote={reconciled:'paper_saved_not_assigned',testno:unassigned.testno};
          }
          records=this.ledger.read();
        }
        const wireSha256=examWireSha256(request.family,this.options.expectedWireHashes);
        const reviewedAt=new Date(this.#now()).toISOString(),expiresAt=new Date(this.#now()+REVIEW_TOKEN_TTL_MS).toISOString();
        const reviewToken=reviewTokenOf({tool:EXAM_TOOL_FAMILY,change,targets:targetStrings,scopeKey,wireSha256,expiresAt});
        this.ledger.append({kind:'reviewed',tool:EXAM_TOOL_FAMILY,reviewToken,changeSha256:changeSha256Of(change),targets:targetStrings,scopeKey,wireSha256,reviewedAt,expiresAt});
        return {status:'reviewed' as const,family:request.family,scopeKey,change,preview:examPreview(change,clsNames,change.steps==='ABC'?change.items:null),reviewToken,wireSha256,reviewedAt,expiresAt,
          ...reconciledNote,
          nextStep:'Show the teacher the preview (exam, students and period); only after the teacher approves, call academy_exam_paper_save with reviewToken, userApproved: true and this change.',
          academyWrites:0,...CLEAN};
      }finally{lock.release();}
    }catch(error){return refused(error);}
  }

  // ---- save -----------------------------------------------------------------------------------------

  async #save(raw:unknown){
    let lock:LedgerLock|null=null,dispatched=false;
    try{
      const input=examPaperSaveSchema.parse(raw);
      if(input.userApproved!==true)throw new GatedWriteRefusal('user_approval_required');          // R3
      const change=qualifyExamChange(input.change);                                                 // R7 (and the shape of R4)
      if(typeof input.reviewToken!=='string')throw new GatedWriteRefusal('review_token_invalid');
      const token=input.reviewToken,targets=sortedTargetStrings(change.targets),scopeKey=examScopeKey(change.family,change.targets),family=change.family;
      if(!this.backend)return {status:'unavailable' as const,code:'exam_source_port_unavailable',reasonCode:'exam_source_port_unavailable',academyWrites:0,...CLEAN};
      this.ledger.assertAvailable();                                                               // R10: before any HTTP request
      const verify=()=>verifyReviewToken(this.ledger.read(),{tool:EXAM_TOOL_FAMILY,token,change,targets,scopeKey,now:this.#now()});
      const reviewed=verify();                                                                      // R4, R5
      if(reviewed.wireSha256!==examWireSha256(family,this.options.expectedWireHashes))throw new GatedWriteRefusal('wire_hash_mismatch');
      const runId=this.#id();
      const overlapping=(a:AttemptRecord)=>a.tool===EXAM_TOOL_FAMILY&&a.scopeKey.startsWith(family+':')&&(a.targets??[]).some(t=>targets.includes(t));
      const taken=this.ledger.acquireLock(runId,false,this.#now);                                   // R11
      if('stale' in taken){
        const records=this.ledger.read();
        const blocking=openAttempts(records).filter(a=>taken.staleRunId===null||a.runId===taken.staleRunId||overlapping(a));
        if(blocking.length)throw new GatedWriteRefusal('reconcile_blocks_retry');
        lock=this.ledger.acquireLock(runId,false,this.#now) as LedgerLock;
      }else lock=taken;
      verify();                                                                                     // the token may have been used while this call waited
      if(openAttempts(this.ledger.read(),{tool:EXAM_TOOL_FAMILY,scopePrefix:family+':',targets}).length)throw new GatedWriteRefusal('reconcile_blocks_retry');

      const attemptId=this.#id(),b=this.backend;
      const progress={writes:0,readBack:'none' as 'none'|'A'|'B'|'C',testno:undefined as string|undefined};
      const base={kind:'attempt' as const,attemptId,tool:EXAM_TOOL_FAMILY,reviewToken:token,changeSha256:reviewed.changeSha256,targets,scopeKey,runId};
      const record=(step:AttemptRecord['step'],state:AttemptState,examStep:ExamStep,extra:Partial<AttemptRecord>={}):AttemptRecord=>
        ({...base,step,state,examStep,at:new Date(this.#now()).toISOString(),...(progress.testno?{testno:progress.testno}:{}),...extra});
      /** A refusal or failure before this step's write: the earlier steps are read back, this step sent nothing. */
      const pre=(error:unknown)=>refused(error,{attemptId,...(progress.testno?{testno:progress.testno}:{}),
        academyWrites:progress.writes,effectStatus:progress.writes===0?'none' as const:'partially_persisted' as const});
      /** A write was sent and not confirmed: the attempt stays open (R14). */
      const unknown=(code:string,status:'unverified'|'refused')=>({status,code,reasonCode:code,failure:publicReadFailure(Error(code),'exam_source_failed'),attemptId,
        ...(progress.testno?{testno:progress.testno}:{}),academyWrites:null,effectStatus:'unknown' as const,...CLEAN,
        nextStep:'The attempt stays open. Call academy_exam_paper_review for the same students: it reconciles by a read-only readback, or answers reconcile_blocks_retry when the LMS cannot show the result. Never retry a save.'});
      const guard=(r:AttemptRecord)=>{try{this.ledger.append(r);}catch(error){throw new ExamStop(pre(error));}};            // before a write: nothing was sent
      const note=(r:AttemptRecord)=>{try{this.ledger.append(r);}catch{throw new ExamStop(unknown('ledger_unavailable','unverified'));}};
      const read=async<T>(run:()=>Promise<T>):Promise<T>=>{try{return await run();}catch(error){if(error instanceof ExamStop)throw error;throw new ExamStop(pre(error));}};

      // A step-C-only attempt records its paper number from the first record: a refusal before its write must still be reconciled to that paper, never to a new one.
      if(change.steps==='C')progress.testno=change.testno;
      guard(record('before_write','open',change.steps==='ABC'?'A':'C'));                         // R10: durable; consumes the token
      let munhangCount:number;
      if(change.steps==='ABC'){
        // ---- step A: stage the items. Its pre-write read is the allocating GET (a write-class request).
        guard(record('write_dispatched','open','A'));
        dispatched=true;progress.writes=1;
        const auto=await b.writeExamGet(autoPath(family));
        const testno=autoPageTestno(auto);
        if(testno){progress.testno=testno;note(record('response_received','open','A'));}
        let page:CreationPage;
        try{
          const observed=examPageSha256('creation',auto);
          if(observed===null||observed!==expectedExamPageHash(family,'creation',this.options.expectedWireHashes))throw new GatedWriteRefusal('wire_hash_mismatch');
          page=parseCreationPage(auto,family,{testgubunNo:change.testgubunNo,courseNo:change.courseNo});
        }catch(error){
          if(!(error instanceof GatedWriteRefusal))throw error;
          // A number was allocated and nothing staged: the attempt resolves absent, with exactly one write-class request sent.
          try{this.ledger.append(record('reconciled','absent','A'));}catch{/* the attempt stays open; a review reconciles it by readback */}
          return refused(error,{attemptId,...(progress.testno?{testno:progress.testno}:{}),academyWrites:1,effectStatus:'absent' as const});
        }
        guard(record('write_dispatched','open','A'));
        progress.writes=2;
        const staged=await b.writeExamGet(saveMunhangPath(family,page.testno,change.testName,change.items));
        const answer=parseSaveMunhang(staged);
        if(answer.kind==='conflict')return unknown('stage_conflict','refused');
        if(answer.kind==='unreadable')return unknown('exam_response_unverified','unverified');
        note(record('response_received','open','A',{responseSha256:sha256Hex(staged)}));
        try{if((await b.readPaperExamList(page.testno)).length!==answer.staged.totalCnt)throw Error('exam_readback_unverified');}
        catch{return unknown('exam_readback_unverified','unverified');}
        progress.readBack='A';
        // ---- step B: save the paper.
        const check=await read(async()=>parseCheckMunhang(await b.readExamRoute(checkMunhangPath(family,page.testno,answer.staged,page.members.find(([n])=>n==='objectviews')![1],this.#now()))));
        if(check.kind==='refused')return pre(new GatedWriteRefusal(check.code));
        guard(record('write_dispatched','open','B'));
        progress.writes=3;
        const saved=await b.writeExamMultipart(paperSavePath(family,change.courseNo),paperSaveMembers(page,change,answer.staged));
        const logic=parseSaveLogic(saved);
        if(logic==='UPLOAD_FAIL')return unknown('save_failed','refused');
        if(logic===null)return unknown('exam_response_unverified','unverified');
        note(record('response_received','open','B',{responseSha256:sha256Hex(saved)}));
        try{
          const modify=parseAutoModify(await b.readExamRoute(autoModifyPath(family,page.testno)));
          if(!modify||modify.testno!==page.testno||modify.testname!==change.testName)throw Error('exam_readback_unverified');
        }catch{return unknown('exam_readback_unverified','unverified');}
        progress.readBack='B';
        munhangCount=answer.staged.totalCnt;
      }else munhangCount=change.munhangCount;
      // ---- step C: assign. Two pre-write reads: the assignment page (hash) and Validate (no target holds the paper yet).
      const testno=progress.testno!;
      const main=await read(()=>b.readExamRoute(assignMainPath(family,testno)));
      const observed=examPageSha256('assignment',main);
      if(observed===null||observed!==expectedExamPageHash(family,'assignment',this.options.expectedWireHashes))return pre(new GatedWriteRefusal('wire_hash_mismatch'));
      const dummy=this.#now();
      const before=await read(async()=>parseValidate(await b.readExamRoute(assignPath(family,'Validate',change,testno,munhangCount,dummy))));
      if(before.kind==='unreadable')return pre(new GatedWriteRefusal('assign_validate_unreadable'));
      if(before.kind==='names')return pre(new GatedWriteRefusal('assign_duplicate'));
      guard(record('write_dispatched','open','C'));
      dispatched=true;progress.writes+=1;
      const assigned=await b.writeExamGet(assignPath(family,'Save',change,testno,munhangCount,dummy));
      if(parseValidate(assigned).kind!=='success')return unknown('assign_failed','refused');
      note(record('response_received','open','C',{responseSha256:sha256Hex(assigned)}));
      try{
        const after=parseValidate(await b.readExamRoute(assignPath(family,'Validate',change,testno,munhangCount,dummy)));
        if(after.kind!=='names'||!validateNamesAll(after.text,change.targets))throw Error('exam_readback_unverified');
      }catch{return unknown('exam_readback_unverified','unverified');}
      progress.readBack='C';
      note(record('reconciled','persisted','C'));
      return {status:'saved' as const,family,scopeKey,attemptId,testno,steps:change.steps,readback:'every_step_confirmed_by_read_only_request',effectStatus:'persisted' as const,
        academyWrites:progress.writes,...CLEAN};
    }catch(error){
      if(error instanceof ExamStop)return error.result;
      // Once a write request may have left, an exception is rethrown so that the audit layers record an unknown effect.
      if(dispatched)throw error;
      return refused(error instanceof z.ZodError?Error('tool_input_invalid'):error);
    }finally{lock?.release();}
  }
}

/** The two tools on the dailyrecord endpoint (src/dailyrecord-server.ts). */
export function registerExamPaperTools(server:McpServer,service:ExamPaperService){
  const result=(value:unknown)=>{const structuredContent=JSON.parse(JSON.stringify(value)) as Record<string,unknown>;
    return {content:[{type:'text' as const,text:JSON.stringify(structuredContent)}],structuredContent};};
  server.registerTool('academy_exam_paper_review',{
    title:'시험지 출제·배정 검토',
    description:academyRoutingDescription('academy_exam_paper_review','Read-only review of an FA or unit-exam paper (family fa or da) and its assignment to named students (targets). Answers the exact change, a preview and a reviewToken (15 minutes).'),
    inputSchema:examPaperReviewSchema,annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:false,openWorldHint:service.backend!==null},
  },async input=>result(await service.review(input)));
  server.registerTool('academy_exam_paper_save',{
    title:'시험지 출제·배정 저장(LMS 쓰기)',
    description:academyRoutingDescription('academy_exam_paper_save','Creates and assigns the exam paper that academy_exam_paper_review reviewed, once, only with the teacher\'s approval (userApproved true). Answers saved after a read-only readback of every step, else unverified; never retry an unverified save.'),
    inputSchema:examPaperSaveSchema,annotations:{readOnlyHint:false,destructiveHint:false,idempotentHint:false,openWorldHint:service.backend!==null},
  },async input=>result(await service.save(input)));
}
