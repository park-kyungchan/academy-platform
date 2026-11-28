import {randomUUID} from 'node:crypto';
import {closeSync,constants,fstatSync,fsyncSync,lstatSync,mkdirSync,openSync,realpathSync,unlinkSync,writeFileSync} from 'node:fs';
import {isAbsolute,resolve} from 'node:path';
import {z} from 'zod';
import type {McpServer} from '@modelcontextprotocol/server';
import {publicReadFailure,readFailureResult} from './read-failure.ts';
import {academyRoutingDescription} from './routing-context.ts';
import {
  GatedWriteLedger,GatedWriteRefusal,REVIEW_TOKEN_TTL_MS,changeSha256Of,openAttempts,reviewTokenOf,sha256Hex,stepsOf,verifyReviewToken,
  type AttemptRecord,type LedgerRecord,type LedgerLock,
} from '../../../../../apps/academy/dc-gang-a/src/lms/gatedWriteLedger';
import {
  CLINIC_MODES,CLINIC_TOOL_FAMILY,CLINIC_WIRE,BOOK_INCORRECT_PAGE,CLINIC_PAGE,clinicReadbackFields,expectedWireHash,parseClinicReadback,parseWriteResponse,
  parseWrongNoteResult,qualifyClinicChange,qualifyClinicRequest,scopeKeyOf,wireSha256OfPage,wrongNoteFields,writeFields,
  type ClinicChange,type ClinicKind,type ExpectedWireHashes,type WireFields,
} from '../../../../../apps/academy/dc-gang-a/src/lms/clinicPaperWrite';

/**
 * `academy_clinic_paper_review` and `academy_clinic_paper_save` (specs/academy-mcp/gated-writes.md): the wrong-note paper of a
 * book and the similar or advance clinic paper of one attempt, made in two phases. The review reads the live page, checks its
 * wire hash, reconciles an unresolved attempt by read-only requests, freezes one change and answers a `reviewToken`; the save
 * writes only that change, only with `userApproved: true`, records each step in the private attempt ledger before the request
 * it guards, and reads the scope back. Every refusal before the write sends 0 write requests (R15). With `printPdf: true` a saved
 * call hands the `print_list` of its own write answer to the paper printer (specs/academy-mcp/pc-paper-executor.md), after the
 * attempt is recorded persisted and outside the serialized section and the ledger lock; the list is never stored. A write answer
 * that `parseWriteResponse` cannot read is kept, raw, as one private evidence file (`keepClinicWriteEvidence`).
 */
/** The raw answer of the write as the port received it: the Content-Type header, the body bytes and their SHA-256. */
export interface ClinicRawAnswer {contentType:string;raw:Uint8Array;sha256:string}
export interface ClinicPaperBackend {
  readClinicWirePage(path:typeof BOOK_INCORRECT_PAGE|typeof CLINIC_PAGE):Promise<string>;
  /** `onRawAnswer`, when given, receives the raw answer besides the text answered; only the save's one write passes it. */
  postTestpageSelectEx(fields:WireFields,onRawAnswer?:(value:ClinicRawAnswer)=>void|Promise<void>):Promise<string>;
}
export interface ClinicPaperOptions {
  now?:()=>number;newId?:()=>string;
  /** Test seam: replaces the pinned production wire hashes so synthetic pages can exercise every path. */
  expectedWireHashes?:ExpectedWireHashes;
  pidAlive?:(pid:number)=>boolean;
  /** The paper printer of `printPdf` (src/paper-print-pdf.ts printClinicList); absent: the PDF part answers pc_executor_not_configured. */
  printer?:ClinicPaperPrinter|null;
  /**
   * The directory of the private evidence of unparsed write answers (production: `<owner artifacts>/clinic-write/http-ack`, made 0700;
   * tests: a mkdtemp directory of mode 0700). None: no file is written and an unparsed answer answers `evidence: 'unavailable'`.
   */
  evidenceRoot?:string|null;
}

const EVIDENCE_NAME=/^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/;
/**
 * Keeps the raw answer of a clinic write that `parseWriteResponse` could not read as `<root>/<attemptId>.json`, holding
 * `{attemptId, scopeKey, contentType, sha256, rawBodyBase64}`: the root is made 0700 when absent and must be a directory of the
 * server's uid, mode 0700, not a symlink, equal to its realpath; the file is created exclusively with mode 0600 (no symlink followed),
 * written whole and synced, then the directory is synced. Throws `clinic_evidence_unavailable` when any of that cannot hold.
 */
export function keepClinicWriteEvidence(root:string,value:{attemptId:string;scopeKey:string;contentType:string;sha256:string;raw:Uint8Array}):void{
  const fail=():never=>{throw Error('clinic_evidence_unavailable');};
  if(!root||!isAbsolute(root)||resolve(root)!==root||root.includes('\0')||!EVIDENCE_NAME.test(value.attemptId))fail();
  mkdirSync(root,{recursive:true,mode:0o700});
  const st=lstatSync(root);
  if(!st.isDirectory()||st.isSymbolicLink()||(st.mode&0o777)!==0o700||st.uid!==process.getuid?.()||realpathSync(root)!==root)fail();
  const path=resolve(root,value.attemptId+'.json');
  const fd=openSync(path,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o600);
  try{
    const file=fstatSync(fd);
    if(!file.isFile()||file.nlink!==1||(file.mode&0o077)!==0||file.uid!==process.getuid?.())fail();
    writeFileSync(fd,JSON.stringify({attemptId:value.attemptId,scopeKey:value.scopeKey,contentType:value.contentType,sha256:value.sha256,
      rawBodyBase64:Buffer.from(value.raw).toString('base64')})+'\n');
    fsyncSync(fd);
  }catch(error){closeSync(fd);try{unlinkSync(path);}catch{/* nothing more can be done */}throw error;}
  closeSync(fd);
  const directory=openSync(root,constants.O_RDONLY);
  try{fsyncSync(directory);}finally{closeSync(directory);}
}
export interface ClinicPaperPrinter {printClinicList(printList:readonly unknown[],keyNo:string):Promise<{status:string;pdfs:unknown[]}&Record<string,unknown>>}

const optional=z.unknown().optional();
/** The review input. Target keys are checked by the service (a missing or non-numeric key is `public_scope_forbidden`, R7), and unknown members are kept so that a public-scope flag is refused by name. */
export const clinicPaperReviewSchema=z.looseObject({
  kind:z.enum(['book_incorrect','clinic']),studentKey:optional,courseKey:optional,bookNo:optional,testingNo:optional,testingNos:optional,
  clinicMode:z.enum(CLINIC_MODES).optional(),createSimilar:z.boolean().optional(),similarCount:z.number().optional(),
  createAdvance:z.boolean().optional(),advanceCount:z.number().optional(),patList:z.array(z.unknown()).optional(),
});
/** The save input: exactly the token, the approval, the reviewed change and optionally `printPdf`. Their values are checked by the service, in the order of R3 to R7. */
export const clinicPaperSaveSchema=z.object({reviewToken:z.string().max(200).optional(),userApproved:z.unknown().optional(),change:z.unknown().optional(),printPdf:z.boolean().optional()}).strict();

const CLEAN={parentSends:0,automaticRetryAllowed:false as const};
interface PrintCarry {printList?:readonly unknown[];keyNo?:string}
const refused=(error:unknown,extra:Record<string,unknown>={})=>{
  const failure=publicReadFailure(error,'clinic_source_failed');
  return {status:'refused' as const,code:failure.code,reasonCode:failure.code,failure,academyWrites:0,...CLEAN,...extra};
};

export class ClinicPaperService {
  #tail:Promise<unknown>=Promise.resolve();
  readonly ledger:GatedWriteLedger;
  constructor(readonly backend:ClinicPaperBackend|null,ledgerPath:string|null,private readonly options:ClinicPaperOptions={}){
    this.ledger=new GatedWriteLedger(ledgerPath,options.pidAlive);
  }
  readonly #now=()=>(this.options.now??Date.now)();
  readonly #id=()=>(this.options.newId??randomUUID)();
  /** One LMS session and one ledger lock serve one call at a time in this process. */
  #serial<T>(run:()=>Promise<T>):Promise<T>{const next=this.#tail.then(run,run);this.#tail=next.catch(()=>undefined);return next;}

  review(raw:unknown){return this.#serial(()=>this.#review(raw));}
  async save(raw:unknown){
    const carry:PrintCarry={};
    const answer=await this.#serial(()=>this.#save(raw,carry));
    if(!(raw!==null&&typeof raw==='object'&&(raw as {printPdf?:unknown}).printPdf===true))return answer;
    // An unverified or refused save does not print. The PDF part runs here, after the persisted ledger record and outside the
    // serialized section and the ledger lock, in its own try/catch: a PC error only adds a pdf member to the saved answer.
    const status=(answer as {status:string}).status;
    if(status!=='saved'&&status!=='persisted')return answer;
    let pdf:Record<string,unknown>;
    try{
      if(status!=='saved'||!carry.printList)pdf={status:'unavailable',reasonCode:'clinic_print_list_unavailable',pdfs:[]};
      else if(!this.options.printer)pdf={status:'unavailable',reasonCode:'pc_executor_not_configured',pdfs:[]};
      else pdf=await this.options.printer.printClinicList(carry.printList,carry.keyNo!);
    }catch(error){pdf={status:'unavailable',...readFailureResult(error,'paper_print_unqualified_no_automatic_retry'),pdfs:[]};}
    finally{delete carry.printList;}
    const paperKeys=Array.isArray((answer as {paperKeys?:unknown}).paperKeys)?(answer as {paperKeys:string[]}).paperKeys:[];
    if(pdf.status!=='rendered')pdf={...pdf,nextStep:paperKeys.length
      ?'The save stands. Print again with academy_paper_print_pdf_get, source paper and each paperKey of this answer, when the LMS lists that paper; never call the save again.'
      :'The save stands; this PDF part cannot be repeated (the answer names no paperKey). Never call the save again.'};
    return {...answer,pdf};
  }

  async #wire(kind:ClinicKind){
    const html=await this.backend!.readClinicWirePage(CLINIC_WIRE[kind].page as typeof BOOK_INCORRECT_PAGE|typeof CLINIC_PAGE);
    const observed=wireSha256OfPage(kind,html),expected=expectedWireHash(kind,this.options.expectedWireHashes);
    if(observed===null||observed!==expected)throw new GatedWriteRefusal('wire_hash_mismatch');
    return observed;
  }
  /** A read-only readback of the scope (0 write requests): the wrong-note list of a book, or the score view of a clinic's attempt. */
  async #readScope(change:{kind:'book_incorrect';studentKey:string;courseKey:string;bookNo:string}|{kind:'clinic';studentKey:string;testingNo:string}):Promise<string[]>{
    if(change.kind==='book_incorrect')return parseWrongNoteResult(await this.backend!.postTestpageSelectEx(wrongNoteFields(change)),change);
    parseClinicReadback(await this.backend!.postTestpageSelectEx(clinicReadbackFields(change)),change.studentKey);
    return [];
  }

  async #review(raw:unknown){
    try{
      const request=qualifyClinicRequest(raw);                       // R7: 0 HTTP requests
      if(!this.backend)return {status:'unavailable' as const,code:'clinic_source_port_unavailable',reasonCode:'clinic_source_port_unavailable',academyWrites:0,...CLEAN};
      this.ledger.assertAvailable();                                  // R10: before any HTTP request
      const runId=this.#id(),studentKey=request.studentKey,scopeKey=scopeKeyOf(request);
      const lock=this.ledger.acquireLock(runId,true,this.#now) as LedgerLock;
      try{
        const wireSha256=await this.#wire(request.kind);              // R1: one read of the live page
        let records=this.ledger.read(),scoped:string[]|null=null;
        const open=openAttempts(records,{tool:CLINIC_TOOL_FAMILY,studentKey,scopeKey});
        if(open.length){
          // R12: a read-only readback decides; a write_dispatched or response_unparsed attempt cannot be decided (the LMS lists no created clinic papers).
          let readable=true;
          try{scoped=await this.#readScope(request.kind==='book_incorrect'?request:{kind:'clinic',studentKey,testingNo:request.testingNo});}catch{readable=false;}
          const outcomes=open.map(a=>{
            const last=stepsOf(records,a.attemptId).at(-1)!;
            if(readable&&last.step==='before_write')return {attempt:last,state:'absent' as const};
            if(readable&&last.step==='response_received'&&last.responseSha256)return {attempt:last,state:'persisted' as const};
            return null;
          });
          if(outcomes.some(o=>o===null))throw new GatedWriteRefusal('reconcile_blocks_retry');
          const settled=outcomes as {attempt:AttemptRecord;state:'absent'|'persisted'}[];
          for(const o of settled)this.ledger.append({...o.attempt,step:'reconciled',state:o.state,at:new Date(this.#now()).toISOString(),runId});
          const done=settled.find(o=>o.state==='persisted');
          if(done)return {status:'persisted' as const,done:true,nothingToSave:true,kind:request.kind,studentKey,scopeKey,attemptId:done.attempt.attemptId,
            responseSha256:done.attempt.responseSha256,paperKeys:done.attempt.paperKeys??[],reconciled:'persisted_by_read_only_readback',academyWrites:0,...CLEAN};
          records=this.ledger.read();
        }
        let change:ClinicChange;
        if(request.kind==='clinic')change=request;
        else{
          let found:string[];
          try{found=scoped??await this.#readScope(request);}catch{throw new GatedWriteRefusal('clinic_scope_unresolved');}
          if(request.testingNos&&!request.testingNos.every(n=>found.includes(n)))throw new GatedWriteRefusal('clinic_scope_unresolved');
          const chosen=request.testingNos??found;
          if(chosen.length<1||chosen.length>50)throw new GatedWriteRefusal('clinic_scope_unresolved');
          change={kind:'book_incorrect',studentKey,courseKey:request.courseKey,bookNo:request.bookNo,testingNos:chosen};
        }
        const reviewedAt=new Date(this.#now()).toISOString(),expiresAt=new Date(this.#now()+REVIEW_TOKEN_TTL_MS).toISOString();
        const reviewToken=reviewTokenOf({tool:CLINIC_TOOL_FAMILY,change,studentKey,scopeKey,wireSha256,expiresAt});
        this.ledger.append({kind:'reviewed',tool:CLINIC_TOOL_FAMILY,reviewToken,changeSha256:changeSha256Of(change),studentKey,scopeKey,wireSha256,reviewedAt,expiresAt});
        const wireForm=writeFields(change);
        return {status:'reviewed' as const,kind:change.kind,studentKey,scopeKey,change,reviewToken,wireSha256,reviewedAt,expiresAt,
          postedParameters:{path:'/servlet/controller.common.TestpageSelectExServlet',p_process:wireForm.p_process,condition:JSON.parse(wireForm.condition!) as unknown},
          nextStep:'Show the teacher the change and postedParameters; only after the teacher approves, call academy_clinic_paper_save with reviewToken, userApproved: true and this change.',
          academyWrites:0,...CLEAN};
      }finally{lock.release();}
    }catch(error){return refused(error);}
  }

  async #save(raw:unknown,carry:PrintCarry){
    let lock:LedgerLock|null=null,dispatched=false;
    try{
      const input=clinicPaperSaveSchema.parse(raw);
      if(input.userApproved!==true)throw new GatedWriteRefusal('user_approval_required');        // R3
      const change=qualifyClinicChange(input.change);                                             // R7 (and the shape of R4)
      if(typeof input.reviewToken!=='string')throw new GatedWriteRefusal('review_token_invalid');
      const token=input.reviewToken,studentKey=change.studentKey,scopeKey=scopeKeyOf(change);
      if(!this.backend)return {status:'unavailable' as const,code:'clinic_source_port_unavailable',reasonCode:'clinic_source_port_unavailable',academyWrites:0,...CLEAN};
      this.ledger.assertAvailable();                                                             // R10: before any HTTP request
      const verify=()=>verifyReviewToken(this.ledger.read(),{tool:CLINIC_TOOL_FAMILY,token,change,studentKey,scopeKey,now:this.#now()});
      const reviewed=verify();                                                                    // R4, R5
      const runId=this.#id();
      const taken=this.ledger.acquireLock(runId,false,this.#now);                                 // R11
      if('stale' in taken){
        // The lock of a dead call was removed. Its unresolved attempt blocks every retry until a review reconciles it.
        const records=this.ledger.read();
        const blocking=openAttempts(records).filter(a=>taken.staleRunId===null||a.runId===taken.staleRunId||(a.studentKey===studentKey&&a.scopeKey===scopeKey));
        if(blocking.length)throw new GatedWriteRefusal('reconcile_blocks_retry');
        lock=this.ledger.acquireLock(runId,false,this.#now) as LedgerLock;
      }else lock=taken;
      verify();                                                                                   // the token may have been used while this call waited
      if(openAttempts(this.ledger.read(),{tool:CLINIC_TOOL_FAMILY,studentKey,scopeKey}).length)throw new GatedWriteRefusal('reconcile_blocks_retry');
      const attemptId=this.#id(),base={kind:'attempt' as const,attemptId,tool:CLINIC_TOOL_FAMILY,reviewToken:token,changeSha256:reviewed.changeSha256,studentKey,scopeKey,runId};
      const record=(step:AttemptRecord['step'],state:AttemptRecord['state'],extra:Partial<AttemptRecord>={}):AttemptRecord=>({...base,step,state,at:new Date(this.#now()).toISOString(),...extra});
      this.ledger.append(record('before_write','open'));                                         // R10: durable; consumes the token
      const markAbsent=()=>{try{this.ledger.append(record('reconciled','absent'));}catch{/* the attempt stays open; a review reconciles it by readback */}};
      try{
        const observed=await this.#wire(change.kind);                                             // R6
        if(observed!==reviewed.wireSha256)throw new GatedWriteRefusal('wire_hash_mismatch');
        this.ledger.append(record('write_dispatched','open'));                                    // durable before the POST
      }catch(error){markAbsent();return refused(error,{attemptId});}
      // From here a failure leaves the attempt open at write_dispatched: the LMS may or may not have made the paper.
      const fields=writeFields(change);
      dispatched=true;
      // The raw answer is held in memory for this call and kept on disk only when its text cannot be parsed.
      const received:{raw?:ClinicRawAnswer}={};
      const answer=await this.backend.postTestpageSelectEx(fields,value=>{received.raw=value;});
      const unverified=(code:string,extra:Record<string,unknown>={})=>({status:'unverified' as const,code,reasonCode:code,failure:publicReadFailure(Error(code),'clinic_source_failed'),
        kind:change.kind,studentKey,scopeKey,attemptId,academyWrites:null,effectStatus:'unknown' as const,...CLEAN,
        nextStep:'The attempt stays open. Call academy_clinic_paper_review for the same target: it reconciles by a read-only readback, or answers reconcile_blocks_retry when the LMS cannot show the result.',...extra});
      let parsed:ReturnType<typeof parseWriteResponse>;
      try{parsed=parseWriteResponse(answer,studentKey);}
      catch{return unverified('clinic_response_unverified',this.#keepUnparsed(answer,received.raw,{attemptId,scopeKey,record}));}
      try{this.ledger.append(record('response_received','open',{responseSha256:parsed.responseSha256,paperKeys:parsed.paperKeys}));}
      catch{return unverified('ledger_unavailable',{printListSha256:parsed.printListSha256,paperKeys:parsed.paperKeys});}
      try{                                                                                         // R14: the read-only scope readback
        const found=await this.#readScope(change.kind==='book_incorrect'?change:{kind:'clinic',studentKey,testingNo:change.testingNo});
        if(change.kind==='book_incorrect'&&!change.testingNos.every(n=>found.includes(n)))throw Error('clinic_readback_unverified');
      }catch{return unverified('clinic_readback_unverified',{printListSha256:parsed.printListSha256,paperKeys:parsed.paperKeys});}
      try{this.ledger.append(record('reconciled','persisted',{responseSha256:parsed.responseSha256,paperKeys:parsed.paperKeys}));}
      catch{return unverified('ledger_unavailable',{printListSha256:parsed.printListSha256,paperKeys:parsed.paperKeys});}
      if(input.printPdf===true){
        // The print list of this write answer, held in memory for the PDF part of this call only; nothing here may throw past the write.
        try{
          const list=(JSON.parse(answer) as {print_list?:unknown}).print_list;
          if(Array.isArray(list))carry.printList=list;
          carry.keyNo=parsed.paperKeys[0]??(change.kind==='clinic'?change.testingNo:change.testingNos[0]!);
        }catch{delete carry.printList;}
      }
      return {status:'saved' as const,kind:change.kind,studentKey,scopeKey,attemptId,paperKeys:parsed.paperKeys,paperCount:parsed.paperCount,printListSha256:parsed.printListSha256,
        readback:'scope_confirmed_by_read_only_request',academyWrites:1,...CLEAN};
    }catch(error){
      // Once the write request may have left, an exception is rethrown so that the audit layers record an unknown effect.
      if(dispatched)throw error;
      return refused(error instanceof z.ZodError?Error('tool_input_invalid'):error);
    }finally{lock?.release();}
  }

  /**
   * An answer `parseWriteResponse` could not read: its raw bytes go to the private evidence file, then the ledger records the step
   * `response_unparsed` with their hash (keys and hashes only, R16). Neither may change the answer's status or the attempt's state
   * (open, indeterminate under R12); a failure of either only shows in the answer as `evidence: 'unavailable'` or a ledger left at
   * `write_dispatched`. No request is sent here.
   */
  #keepUnparsed(text:string,raw:ClinicRawAnswer|undefined,at:{attemptId:string;scopeKey:string;record:(step:AttemptRecord['step'],state:AttemptRecord['state'],extra?:Partial<AttemptRecord>)=>AttemptRecord}){
    const responseSha256=raw?sha256Hex(raw.raw):sha256Hex(text);
    let evidence:'kept'|'unavailable'='unavailable';
    if(raw&&this.options.evidenceRoot){
      try{keepClinicWriteEvidence(this.options.evidenceRoot,{attemptId:at.attemptId,scopeKey:at.scopeKey,contentType:raw.contentType,sha256:responseSha256,raw:raw.raw});evidence='kept';}
      catch{/* evidence: 'unavailable' */}
    }
    try{this.ledger.append(at.record('response_unparsed','open',{responseSha256}));}catch{/* the attempt stays open at write_dispatched, also indeterminate */}
    return {responseSha256,evidence};
  }
}

/** The two tools on the dailyrecord endpoint (src/dailyrecord-server.ts). */
export function registerClinicPaperTools(server:McpServer,service:ClinicPaperService){
  const result=(value:unknown)=>{const structuredContent=JSON.parse(JSON.stringify(value)) as Record<string,unknown>;
    return {content:[{type:'text' as const,text:JSON.stringify(structuredContent)}],structuredContent};};
  server.registerTool('academy_clinic_paper_review',{
    title:'오답·클리닉지 검토',
    description:academyRoutingDescription('academy_clinic_paper_review','Read-only review of a wrong-note or clinic paper: kind book_incorrect (studentKey, courseKey, bookNo) or clinic (studentKey, testingNo, clinicMode multiply/divide/custom, createSimilar, createAdvance). Answers the exact change and a reviewToken (15 minutes).'),
    inputSchema:clinicPaperReviewSchema,annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:false,openWorldHint:service.backend!==null},
  },async input=>result(await service.review(input)));
  server.registerTool('academy_clinic_paper_save',{
    title:'오답·클리닉지 저장(LMS 쓰기)',
    description:academyRoutingDescription('academy_clinic_paper_save','Creates the wrong-note or clinic paper that academy_clinic_paper_review reviewed, once, only with the teacher\'s approval (userApproved true). Answers saved after a read-only scope readback, else unverified; never retry an unverified save. printPdf true: a saved call also prints the papers of its write answer on the PC (pdf member).'),
    inputSchema:clinicPaperSaveSchema,annotations:{readOnlyHint:false,destructiveHint:false,idempotentHint:false,openWorldHint:service.backend!==null},
  },async input=>result(await service.save(input)));
}
