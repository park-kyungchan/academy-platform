import type {TeacherApplication} from '../application';
import type {DateClassRow} from './dateClassView';
import {occurrenceIdentity} from './dateClassView';
type Scene=Awaited<ReturnType<TeacherApplication['student']>>;
type Receipt=Awaited<ReturnType<TeacherApplication['saveObservation']>>;
interface Input {
  studentId:string;occurrence:DateClassRow['occurrence'];basisRevisionId:string;expectedRevisionId:string|null;
  idempotencyKey:string;reportedAt:string;text:string;
}
interface Draft {text:string;expectedRevisionId:string|null;headSequence:number;basisRevisionId:string;pending:Input|null;message:string;blocked:boolean;dirty:boolean;retryAllowed:boolean;recovering:Input|null;listeners:Set<()=>void>}
// Page-lifetime drafts only: no teacher originals in browser localStorage or shared fixtures.
const drafts=new Map<string,Draft>();
const node=(tag:string,text='')=>{const el=document.createElement(tag);el.textContent=text;return el;};
export function observationEditor(scene:Scene,row:DateClassRow|null,read:<T>(path:string)=>Promise<T>,write:<T>(path:string,input:unknown)=>Promise<T>,lifetime?:AbortSignal):HTMLElement{
  const card=node('section');card.className='card';card.append(node('h3','교사 관찰 · 원문과 수정 이력'));
  card.append(node('p','개인 교사 관찰입니다. 공식 기록 저장·승인·부모 전송이 아닙니다.'));
  if(!row||row.studentId!==scene.student.id||!scene.evidence.some(v=>v.revisionId===row.revisionId&&v.payload.kind==='source_audit')){
    card.append(node('p','날짜별 수업과 현재 출처 근거를 선택한 뒤 기록할 수 있습니다.'));return card;
  }
  const key=occurrenceIdentity(row);
  const prior=scene.evidence.find(v=>v.payload.kind==='teacher_observation'&&Object.entries(row.occurrence).every(([k,value])=>v.payload.kind==='teacher_observation'&&v.payload.occurrence[k as keyof typeof row.occurrence]===value));
  let draft=drafts.get(key);
  if(!draft){draft={text:prior?.payload.kind==='teacher_observation'?prior.payload.text:'',expectedRevisionId:prior?.revisionId??null,headSequence:prior?.sequence??0,basisRevisionId:row.revisionId,pending:null,message:'',blocked:false,dirty:false,retryAllowed:false,recovering:null,listeners:new Set()};drafts.set(key,draft);}
  // Student queries and save acknowledgements are separate clocks: a precommit snapshot is not a newer head.
  else if(!draft.pending&&(draft.headSequence===0||prior&&(prior.revisionId===draft.expectedRevisionId||prior.sequence>draft.headSequence))){
    const changed=draft.expectedRevisionId!==(prior?.revisionId??null)||draft.basisRevisionId!==row.revisionId;
    if(changed&&(draft.dirty||draft.blocked)){draft.blocked=true;draft.message='저장된 관찰 또는 출처 근거가 변경됐습니다. 내 입력은 보존했습니다. 현재 근거를 확인한 뒤 명시적으로 수정을 계속하세요.';}
    else if(changed){draft.text=prior?.payload.kind==='teacher_observation'?prior.payload.text:'';draft.expectedRevisionId=prior?.revisionId??null;draft.headSequence=prior?.sequence??0;draft.basisRevisionId=row.revisionId;draft.message='';}
  }
  const state=draft;
  const label=node('label','선택 수업의 교사 관찰');label.setAttribute('for','teacher-observation-text');
  const text=document.createElement('textarea');text.id='teacher-observation-text';text.maxLength=16000;text.rows=5;text.value=state.text;text.style.cssText='width:100%;font:inherit;resize:vertical';
  const save=node('button','관찰 저장 / 수정') as HTMLButtonElement;save.id='teacher-observation-save';save.type='button';
  const recover=node('button','원본 요청의 저장 결과 확인') as HTMLButtonElement;recover.id='teacher-observation-recover';recover.type='button';
  const retry=node('button','같은 원본 요청으로 재시도') as HTMLButtonElement;retry.id='teacher-observation-retry';retry.type='button';
  const reconcile=node('button','현재 근거를 확인하고 내 입력으로 수정 계속') as HTMLButtonElement;reconcile.id='teacher-observation-reconcile';reconcile.type='button';
  const current=node('p','현재 저장된 관찰: '+(prior?.payload.kind==='teacher_observation'?prior.payload.text:'없음')+' · 관찰 revision '+(prior?.revisionId??'없음')+' · 출처 revision '+row.revisionId);

  const message=node('p');message.id='teacher-observation-status';message.setAttribute('role','status');message.setAttribute('aria-live','polite');message.style.overflowWrap='anywhere';
  const sync=()=>{text.readOnly=!!state.pending;save.disabled=!!state.pending||state.blocked||!state.text.trim();recover.hidden=!state.pending;recover.disabled=!!state.pending&&state.recovering===state.pending;retry.hidden=!state.pending||!state.retryAllowed;reconcile.hidden=!state.blocked||!!state.pending;current.hidden=reconcile.hidden;message.textContent=state.message;};
  if(!lifetime?.aborted)state.listeners.add(sync);
  lifetime?.addEventListener('abort',()=>state.listeners.delete(sync),{once:true});
  const notify=()=>{for(const listener of state.listeners)listener();};
  const accept=(receipt:Pick<Receipt,'revisionId'|'record'|'officialSaved'|'parentSent'>,input:Input)=>{
    if(state.pending!==input)return;
    const record=receipt.record,payload=record?.payload;
    if(receipt.officialSaved!==false||receipt.parentSent!==false||record?.studentId!==input.studentId||!Number.isSafeInteger(record.sequence)||record.sequence<1||record.revisionId!==receipt.revisionId||record.idempotencyKey!==input.idempotencyKey||record.previousRevisionId!==input.expectedRevisionId||payload?.kind!=='teacher_observation'||payload.text!==input.text||payload.basisRevisionId!==input.basisRevisionId||payload.reportedAt!==input.reportedAt||Object.entries(input.occurrence).some(([k,v])=>payload.occurrence[k as keyof typeof input.occurrence]!==v))throw Error('observation_readback_conflict');
    state.expectedRevisionId=receipt.revisionId;state.headSequence=record.sequence;state.pending=null;state.message='저장 확인 · '+receipt.revisionId+' · 원문과 이전 수정 이력은 보존됩니다.';state.blocked=false;state.dirty=false;
  };
  text.addEventListener('input',()=>{state.text=text.value;state.dirty=true;notify();});
  reconcile.addEventListener('click',()=>{
    if(state.pending||!state.blocked)return;
    state.expectedRevisionId=prior?.revisionId??null;state.headSequence=prior?.sequence??0;state.basisRevisionId=row.revisionId;state.blocked=false;state.message='현재 근거를 확인했습니다. 내 입력은 유지하며 다음 저장은 새 수정 요청입니다.';notify();
  });
  const submit=async(input:Input)=>{
    state.retryAllowed=false;
    state.pending=input;state.message='저장 결과 확인 중…';notify();
    try{accept(await write<Receipt>('/api/teacher-observation',input),input);}
    catch(error){
      if(state.pending!==input)return;
      const reason=(error as Error).message;
      if(reason==='input_too_large'){
        state.pending=null;state.message='저장 전 크기 제한으로 거부됐습니다. 입력은 보존했습니다. 내용을 줄여 새 요청으로 저장할 수 있습니다.';
      }else if(['stale_base','stale_evidence_basis','observation_basis_conflict','idempotency_reuse'].includes(reason)){
        state.pending=null;state.blocked=true;state.message='저장 거부 · '+reason+' · 입력은 보존했습니다. 수업 근거를 다시 조회한 뒤 내용을 검토하세요.';
      }else state.message='저장 응답을 확인하지 못했습니다. 입력과 원본 요청을 보존했습니다. 다시 저장하지 말고 저장 결과를 확인하세요.';
    }finally{notify();}
  };
  save.addEventListener('click',()=>{
    if(state.pending||state.blocked||!state.text.trim())return;
    void submit({studentId:scene.student.id,occurrence:structuredClone(row.occurrence),basisRevisionId:state.basisRevisionId,expectedRevisionId:state.expectedRevisionId,idempotencyKey:crypto.randomUUID(),reportedAt:new Date().toISOString(),text:state.text});
  });
  retry.addEventListener('click',()=>{if(state.pending&&state.retryAllowed)void submit(state.pending);});
  recover.addEventListener('click',async()=>{
    const input=state.pending;if(!input||state.recovering===input)return;state.recovering=input;notify();
    try{accept(await read<Receipt>('/api/teacher-observation-result?studentId='+encodeURIComponent(input.studentId)+'&requestId='+encodeURIComponent(input.idempotencyKey)),input);}
    catch(error){if(state.pending!==input)return;state.retryAllowed=(error as Error).message==='observation_result_not_found';state.message='원본 요청의 저장 결과가 아직 확인되지 않았습니다. 입력을 유지하고 새 요청은 보내지 않습니다.';}
    finally{if(state.recovering===input)state.recovering=null;notify();}
  });
  card.append(label,text,save,recover,retry,current,reconcile,message);sync();return card;
}
