import {expect,test} from 'bun:test';
import {readFileSync} from 'node:fs';
import {digest} from '../../src/learning/model';
import {parentInternalLanguage} from '../../src/contracts/parentInternalLanguage';
import {validateTeacherWorkspace,validateTeacherWorkspaceWrite,type TeacherParentDraft} from '../../src/academy/teacherWorkspace';
import {appendOwnerWorkspace,ownerWorkspaceVersionFrom} from '../../src/academy/ownerWorkspaceStore';
import {workspaceEntityId} from '../../src/academy/teacherWorkspace';

const binding={studentId:'fictional-student',sourceStudent:{namespace:'academy_student',literalKey:'77001'}};
function legacyRow(){
 const payload:TeacherParentDraft={kind:'teacher_parent_draft',studentBinding:binding,sourceStudent:binding.sourceStudent,reportedAt:'2025-01-02T01:00:00.000Z',evidenceRevisionIds:['fictional-basis'],date:'2025-01-02',state:'draft',progress:[{text:'가우스 시리즈2 v1(1SAG) 준비',evidenceRevisionIds:['fictional-basis']}],homework:[],memo:[],internalNote:'',teacherReviewed:false,officialSaved:false,parentSent:false,deliveryReceipt:'unobserved'};
 const body={ownerId:'fictional-owner',entityId:workspaceEntityId(payload),sequence:1,revisionId:'fictional-legacy-revision',previousRevisionId:null,previousDigest:null,payload,contentDigest:digest(payload),recordedAt:'2025-01-02T01:00:00.000Z',actorId:'fictional-teacher',idempotencyKey:'fictional-old-command',commandDigest:'a'.repeat(64)};
 return {owner_id:body.ownerId,entity_id:body.entityId,sequence:body.sequence,revision_id:body.revisionId,previous_revision_id:null,previous_digest:null,payload,content_digest:body.contentDigest,integrity_digest:digest(body),recorded_at:body.recordedAt,actor_id:body.actorId,idempotency_key:body.idempotencyKey,command_digest:body.commandDigest};
}

test('stored legacy draft remains readable with exact text and integrity checks',()=>{
 const row=legacyRow(),before=structuredClone(row);
 const stored=ownerWorkspaceVersionFrom(row);
 expect(stored.payload).toEqual(before.payload);expect(row).toEqual(before);
 expect(()=>validateTeacherWorkspace(row.payload)).not.toThrow();
 expect(()=>validateTeacherWorkspaceWrite(row.payload)).toThrow('parent_internal_language_excluded');
 const corrupt=structuredClone(row);corrupt.payload.progress[0]!.text='Changed historical text';
 expect(()=>ownerWorkspaceVersionFrom(corrupt)).toThrow('stored_owner_workspace_integrity');
 expect(()=>ownerWorkspaceVersionFrom({...row,integrity_digest:'0'.repeat(64)})).toThrow('stored_owner_workspace_integrity');
});

test('direct append refuses excluded new wording before entering the storage transaction',async()=>{
 const row=legacyRow();let transactions=0;
 const db={begin:()=>{transactions++;throw Error('must not open storage');}};
 await expect(appendOwnerWorkspace(db as never,{ownerId:'fictional-owner',actorId:'fictional-teacher',idempotencyKey:'fictional-new-command',expectedRevisionId:row.revision_id,payload:row.payload})).rejects.toThrow('parent_internal_language_excluded');
 expect(transactions).toBe(0);
});

test('a compliant revision can append over a stored legacy draft without rewriting it',async()=>{
 const row=legacyRow(),before=JSON.stringify(row),payload=structuredClone(row.payload);
 payload.reportedAt='2025-01-03T01:00:00.000Z';payload.progress[0]!.text='가우스 5-1-2 준비';
 let inserts=0;
 const tx=async(strings:TemplateStringsArray)=>{
  const query=strings.join('?');
  if(query.includes('SELECT source_namespace,source_key'))return [{source_namespace:'academy_student',source_key:'77001'}];
  if(query.includes('SELECT v.revision_id'))return [{revision_id:'fictional-basis'}];
  if(query.includes('SELECT v.* FROM academy.owner_workspace_heads'))return [row];
  if(query.includes('INSERT INTO academy.owner_workspace_versions'))inserts++;
  return [];
 };
 const db={begin:(run:(tx:unknown)=>unknown)=>run(tx)};
 const result=await appendOwnerWorkspace(db as never,{ownerId:'fictional-owner',actorId:'fictional-teacher',idempotencyKey:'fictional-corrected-command',expectedRevisionId:row.revision_id,payload});
 expect(result.status).toBe('saved');expect(result.sequence).toBe(2);expect(inserts).toBe(1);
 expect(JSON.stringify(row)).toBe(before);
});

test('parent guard uses exactly the observed cover codes and allows ordinary algebra',()=>{
 const catalog=JSON.parse(readFileSync(new URL('../../research/textbooks/edition-catalog-20261028-v2.json',import.meta.url),'utf8'));
 const codes=[...new Set<string>(catalog.books.map((book:{identity:{printedCoverCode:string}})=>book.identity.printedCoverCode))].sort();
 const declared=parentInternalLanguage.source.match(/\(\?:((?:[0-9A-Z]+\|)*[0-9A-Z]+)\)/)![1]!.split('|').sort();
 expect(declared).toEqual(codes);
 for(const code of codes)expect(parentInternalLanguage.test(`- 가우스 5-1-2 (${code}) 준비`)).toBe(true);
 for(const text of ['- 2x3y의 계산 연습','- 다항식 3abc 계산','- 가우스 5-1-2 준비'])expect(parentInternalLanguage.test(text)).toBe(false);
});
