import type { SQL } from 'bun';
import { assertExactKeys, assertInstant, assertRefs, canonicalJson, digest, requireText, stableId } from '../learning/model';
import { validateHierarchy, validateSourceKey, type CatalogNode, type SourceKey } from './domain';
import { createAcademySourceAudit } from '../lms/academySourceAudit';
import { readInputRecord } from '../lms/academyReadProtocol';
import { validateAcademyRecord, type AcademyRecord } from './records';
import {assertPreparationDate} from './preparationHistory';

export interface StudentRegistration {
  readonly ownerId: string; readonly id: string; readonly displayName: string;
  readonly sourceKey: SourceKey; readonly proofRefs: readonly string[];
}
export interface StoredStudent extends StudentRegistration { readonly registeredAt: string }
interface StudentRow { id: string; display_name: string; source_namespace: string; source_key: string; proof_refs: string[]; registered_at: Date | string }
export interface AppendCommand {
  readonly ownerId: string; readonly studentId: string; readonly entityId: string; readonly actorId: string;
  readonly idempotencyKey: string; readonly expectedRevisionId: string | null; readonly sourceObservedAt: string; readonly payload: AcademyRecord;
}
export interface WriteAcknowledgement { readonly status: 'saved' | 'unchanged' | 'replay'; readonly revisionId: string; readonly sequence: number; readonly recordedAt: string; readonly contentDigest: string; readonly officialSaved: false; readonly parentSent: false }
export interface RecordVersion {
  ownerId: string; studentId: string; entityId: string; sequence: number; revisionId: string;
  previousRevisionId: string | null; previousDigest: string | null; payload: AcademyRecord;
  contentDigest: string; integrityDigest: string; sourceObservedAt: string; recordedAt: string;
  actorId: string; idempotencyKey: string; commandDigest: string;
}
interface VersionRow { owner_id:string;student_id:string;entity_id:string;sequence:bigint|number;revision_id:string;previous_revision_id:string|null;previous_digest:string|null;payload:AcademyRecord;content_digest:string;integrity_digest:string;source_observed_at:Date|string;recorded_at:Date|string;actor_id:string;idempotency_key:string;command_digest:string }
function versionFrom(row: VersionRow): RecordVersion {
  const value: RecordVersion = { ownerId:row.owner_id,studentId:row.student_id,entityId:row.entity_id,sequence:Number(row.sequence),revisionId:row.revision_id,
    previousRevisionId:row.previous_revision_id,previousDigest:row.previous_digest,payload:row.payload,contentDigest:row.content_digest,integrityDigest:row.integrity_digest,
    sourceObservedAt:new Date(row.source_observed_at).toISOString(),recordedAt:new Date(row.recorded_at).toISOString(),actorId:row.actor_id,idempotencyKey:row.idempotency_key,commandDigest:row.command_digest };
  validateAcademyRecord(value.payload);
  const {integrityDigest,...body}=value;
  if (!Number.isSafeInteger(value.sequence)||value.sequence<1||digest(body)!==integrityDigest) throw Error('stored_revision_integrity');
  return value;
}
/** Trusted provider adapter; caller authentication and operation grants belong to application ingress. */
export class PostgresAcademyStore {
  constructor(readonly db: SQL) {}
  /** Startup qualification only; never repairs or migrates a production store. */
  async assertRuntimeReady():Promise<void> {
    const [row]=await this.db`
      SELECT to_regclass('academy.students') IS NOT NULL
        AND to_regclass('academy.record_versions') IS NOT NULL
        AND to_regclass('academy.record_heads') IS NOT NULL
        AND to_regclass('academy.command_outcomes') IS NOT NULL
        AND to_regclass('academy.catalog_versions') IS NOT NULL
        AND to_regclass('academy.source_catalog_links') IS NOT NULL AS tables_ready,
        (SELECT count(*)::int FROM pg_catalog.pg_trigger t
          JOIN pg_catalog.pg_class c ON c.oid=t.tgrelid
          JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
          JOIN pg_catalog.pg_proc p ON p.oid=t.tgfoid
          JOIN pg_catalog.pg_namespace pn ON pn.oid=p.pronamespace
          WHERE n.nspname='academy' AND pn.nspname='academy'
            AND p.proname='prevent_history_mutation' AND NOT t.tgisinternal
            AND t.tgenabled='O' AND t.tgtype=27
            AND ((c.relname='record_versions' AND t.tgname='immutable_history')
              OR (c.relname='catalog_versions' AND t.tgname='immutable_catalog'))) AS history_triggers`;
    if(row?.tables_ready!==true||row.history_triggers!==2)throw Error('existing_academy_schema_required');
  }
  async migrate(): Promise<void> {
    await this.db.unsafe(`
      CREATE SCHEMA IF NOT EXISTS academy;
      CREATE TABLE IF NOT EXISTS academy.students (
        owner_id text NOT NULL, id text NOT NULL, display_name text NOT NULL,
        source_namespace text NOT NULL, source_key text NOT NULL, proof_refs jsonb NOT NULL,
        registered_at timestamptz NOT NULL DEFAULT clock_timestamp(),
        PRIMARY KEY(owner_id,id), UNIQUE(owner_id,source_namespace,source_key),
        CHECK(length(display_name)>0), CHECK(jsonb_typeof(proof_refs)='array' AND jsonb_array_length(proof_refs)>0)
      );
      CREATE TABLE IF NOT EXISTS academy.record_versions (
        owner_id text NOT NULL, student_id text NOT NULL, entity_id text NOT NULL,
        sequence bigint NOT NULL CHECK(sequence>0), revision_id text NOT NULL,
        previous_revision_id text, previous_digest text, payload jsonb NOT NULL,
        content_digest text NOT NULL, integrity_digest text NOT NULL,
        source_observed_at timestamptz NOT NULL, recorded_at timestamptz NOT NULL,
        actor_id text NOT NULL, idempotency_key text NOT NULL, command_digest text NOT NULL,
        PRIMARY KEY(owner_id,student_id,entity_id,sequence),
        UNIQUE(owner_id,student_id,revision_id),
        FOREIGN KEY(owner_id,student_id) REFERENCES academy.students(owner_id,id)
      );
      CREATE TABLE IF NOT EXISTS academy.record_heads (
        owner_id text NOT NULL, student_id text NOT NULL, entity_id text NOT NULL,
        revision_id text NOT NULL, sequence bigint NOT NULL, content_digest text NOT NULL,
        last_verified_at timestamptz NOT NULL,
        PRIMARY KEY(owner_id,student_id,entity_id),
        FOREIGN KEY(owner_id,student_id,entity_id,sequence) REFERENCES academy.record_versions(owner_id,student_id,entity_id,sequence)
      );
      CREATE TABLE IF NOT EXISTS academy.command_outcomes (
        owner_id text NOT NULL, actor_id text NOT NULL, idempotency_key text NOT NULL,
        command_digest text NOT NULL, acknowledgement jsonb NOT NULL,
        PRIMARY KEY(owner_id,actor_id,idempotency_key)
      );
      CREATE TABLE IF NOT EXISTS academy.catalog_versions (
        owner_id text NOT NULL, node_id text NOT NULL, sequence bigint NOT NULL,
        revision_id text NOT NULL, payload jsonb NOT NULL, content_digest text NOT NULL,
        recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
        PRIMARY KEY(owner_id,node_id,sequence), UNIQUE(owner_id,revision_id)
      );
      CREATE TABLE IF NOT EXISTS academy.source_catalog_links (
        owner_id text NOT NULL, student_id text NOT NULL, source_revision_id text NOT NULL, catalog_revision_id text NOT NULL,
        relation_type text NOT NULL CHECK(relation_type='source_book_reference'),
        PRIMARY KEY(owner_id,student_id,source_revision_id,catalog_revision_id),
        FOREIGN KEY(owner_id,student_id,source_revision_id) REFERENCES academy.record_versions(owner_id,student_id,revision_id),
        FOREIGN KEY(owner_id,catalog_revision_id) REFERENCES academy.catalog_versions(owner_id,revision_id)
      );
      CREATE INDEX IF NOT EXISTS record_history_clock ON academy.record_versions(owner_id,student_id,recorded_at,entity_id,sequence);
      CREATE OR REPLACE FUNCTION academy.prevent_history_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
      BEGIN RAISE EXCEPTION 'append_only_history'; END; $$;
      DROP TRIGGER IF EXISTS immutable_history ON academy.record_versions;
      CREATE TRIGGER immutable_history BEFORE UPDATE OR DELETE ON academy.record_versions FOR EACH ROW EXECUTE FUNCTION academy.prevent_history_mutation();
      DROP TRIGGER IF EXISTS immutable_catalog ON academy.catalog_versions;
      CREATE TRIGGER immutable_catalog BEFORE UPDATE OR DELETE ON academy.catalog_versions FOR EACH ROW EXECUTE FUNCTION academy.prevent_history_mutation();
      REVOKE ALL ON SCHEMA academy FROM PUBLIC;
      REVOKE ALL ON ALL TABLES IN SCHEMA academy FROM PUBLIC;
    `);
  }
  async registerStudent(input: StudentRegistration): Promise<void> {
    assertExactKeys(input, ['ownerId', 'id', 'displayName', 'sourceKey', 'proofRefs']);
    canonicalJson(input); validateSourceKey(input.sourceKey); assertRefs(input.proofRefs);
    for (const field of [input.ownerId, input.id, input.displayName]) requireText(field, 'student registration');
    if (input.displayName.length > 200 || input.sourceKey.namespace !== 'academy_student') throw Error('student_binding_contract');
    await this.db.begin(async tx => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${canonicalJson([input.ownerId,input.id])},0))`;
      await tx`INSERT INTO academy.students(owner_id,id,display_name,source_namespace,source_key,proof_refs)
        VALUES(${input.ownerId},${input.id},${input.displayName},${input.sourceKey.namespace},${input.sourceKey.literalKey},${canonicalJson(input.proofRefs)}::text::jsonb)
        ON CONFLICT(owner_id,id) DO NOTHING`;
      const [row] = await tx`SELECT display_name,source_namespace,source_key FROM academy.students WHERE owner_id=${input.ownerId} AND id=${input.id}`;
      if (!row || row.display_name !== input.displayName || row.source_namespace !== input.sourceKey.namespace || row.source_key !== input.sourceKey.literalKey) throw Error('student_rebinding_requires_revision');
    });
  }
  async students(ownerId: string): Promise<StoredStudent[]> {
    requireText(ownerId, 'owner');
    const rows = await this.db`SELECT id,display_name,source_namespace,source_key,proof_refs,registered_at FROM academy.students WHERE owner_id=${ownerId} ORDER BY id`;
    return rows.map((row: StudentRow) => ({ ownerId, id: row.id, displayName: row.display_name,
      sourceKey: { namespace: row.source_namespace, literalKey: row.source_key }, proofRefs: row.proof_refs,
      registeredAt: new Date(row.registered_at).toISOString() }));
  }
  async append(command: AppendCommand, recordedAt = new Date().toISOString()): Promise<WriteAcknowledgement> {
    assertExactKeys(command,['ownerId','studentId','entityId','actorId','idempotencyKey','expectedRevisionId','sourceObservedAt','payload']);
    canonicalJson(command); validateAcademyRecord(command.payload);
    for(const value of [command.ownerId,command.studentId,command.entityId,command.actorId,command.idempotencyKey])requireText(value,'append identity');
    if(command.expectedRevisionId!==null)requireText(command.expectedRevisionId,'expected revision');
    assertInstant(command.sourceObservedAt);assertInstant(recordedAt);
    if(Date.parse(command.sourceObservedAt)>Date.parse(recordedAt))throw Error('future_source_observation');
    const commandDigest=digest(command);
    const contentDigest=command.payload.kind==='source_audit'?createAcademySourceAudit(command.payload.acquisition).contentDigest:digest(command.payload);
    const acknowledgement=await this.db.begin(async tx=>{
      // Serialize source replacement and source-bound teacher intake in the same student partition.
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${canonicalJson(['student-records',command.ownerId,command.studentId])},0))`;
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${canonicalJson([command.ownerId,command.actorId,command.idempotencyKey])},0))`;
      const [retry]=await tx`SELECT command_digest,acknowledgement FROM academy.command_outcomes WHERE owner_id=${command.ownerId} AND actor_id=${command.actorId} AND idempotency_key=${command.idempotencyKey}`;
      if(retry){if(retry.command_digest!==commandDigest)throw Error('idempotency_reuse');return {...retry.acknowledgement,status:'replay'} as WriteAcknowledgement;}
      const [student]=await tx`SELECT source_key FROM academy.students WHERE owner_id=${command.ownerId} AND id=${command.studentId}`;
      if(!student)throw Error('unknown_student_partition');
      if('sourceStudent' in command.payload&&command.payload.sourceStudent.literalKey!==student.source_key)throw Error('source_student_binding_conflict');
      if(command.payload.kind==='teacher_observation'){
        const payload=command.payload;
        if(payload.reportedAt!==command.sourceObservedAt)throw Error('observation_clock_conflict');
        const [basis]=await tx`SELECT v.* FROM academy.record_heads h JOIN academy.record_versions v USING(owner_id,student_id,entity_id,sequence)
          WHERE h.owner_id=${command.ownerId} AND h.student_id=${command.studentId} AND v.revision_id=${payload.basisRevisionId}`;
        if(!basis)throw Error('stale_evidence_basis');
        const version=versionFrom(basis);
        if(version.payload.kind!=='source_audit')throw Error('observation_basis_conflict');
        const snapshot=readInputRecord(version.payload.acquisition);
        if(snapshot.family!=='day_record'||!Array.isArray(snapshot.dayRecords)||!snapshot.dayRecords.some(raw=>{
          const row=readInputRecord(raw);return row.studentKey===student.source_key&&Object.entries(payload.occurrence).every(([key,value])=>row[key]===value);
        }))throw Error('observation_basis_conflict');
      }
      if(command.payload.kind==='source_course_inventory'||command.payload.kind==='teacher_activity_revision'||command.payload.kind==='teacher_preparation_assignment'){
        const payload=command.payload;
        const registrations=await tx`SELECT v.payload FROM academy.record_heads h JOIN academy.record_versions v USING(owner_id,student_id,entity_id,sequence)
          WHERE h.owner_id=${command.ownerId} AND h.student_id=${command.studentId} AND v.payload->>'kind'='source_book_registration'`;
        const matches=registrations.filter((r:{payload:AcademyRecord})=>r.payload.kind==='source_book_registration'&&canonicalJson(r.payload.sourceCourse)===canonicalJson(payload.sourceCourse)&&canonicalJson(r.payload.sourceBook)===canonicalJson(payload.sourceBook)&&(!('sourceUserCourse' in payload)||canonicalJson(r.payload.sourceUserCourse)===canonicalJson(payload.sourceUserCourse)));
        if(matches.length!==1)throw Error('preparation_book_binding_conflict');
      }
      let catalogRevisionId:string|null=null;
      if(command.payload.kind==='source_book_registration'){
        await tx`SELECT pg_advisory_xact_lock(hashtextextended(${canonicalJson([command.ownerId,'catalog-write'])},0))`;
        const [catalog]=await tx`SELECT revision_id,payload FROM academy.catalog_versions WHERE owner_id=${command.ownerId} AND node_id=${command.payload.catalogNodeId} ORDER BY sequence DESC LIMIT 1`;
        if(!catalog||catalog.payload.kind!=='book_edition'||canonicalJson(catalog.payload.sourceKey)!==canonicalJson(command.payload.sourceBook))throw Error('catalog_reference_conflict');
        catalogRevisionId=catalog.revision_id;
      }
      if(command.payload.kind==='source_audit'){
        const audit=createAcademySourceAudit(command.payload.acquisition);
        if(Date.parse(audit.observedAt)!==Date.parse(command.sourceObservedAt))throw Error('source_observation_clock_conflict');
        if(audit.entities.some(entity=>'studentKey' in entity.sourceIdentity&&entity.sourceIdentity.studentKey!==student.source_key))throw Error('source_student_binding_conflict');
        const snapshot=readInputRecord(command.payload.acquisition);
        if(snapshot.family==='item_metadata')throw Error('shared_metadata_requires_catalog_partition');
        const targetKeys:string[]=[];
        for(const field of ['records','dailyResults','dayRecords'])if(Array.isArray(snapshot[field]))for(const row of snapshot[field]){const target=readInputRecord(row);if(typeof target.studentKey==='string')targetKeys.push(target.studentKey);}
        if(snapshot.studentCourseScope){const scope=readInputRecord(snapshot.studentCourseScope);if(typeof scope.studentKey==='string')targetKeys.push(scope.studentKey);}
        if(targetKeys.some(key=>key!==student.source_key))throw Error('source_student_binding_conflict');
      }
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${canonicalJson([command.ownerId,command.studentId,command.entityId])},0))`;
      const [head]=await tx`SELECT v.* FROM academy.record_heads h JOIN academy.record_versions v USING(owner_id,student_id,entity_id,sequence)
        WHERE h.owner_id=${command.ownerId} AND h.student_id=${command.studentId} AND h.entity_id=${command.entityId}`;
      const prior=head?versionFrom(head):null;
      if(command.expectedRevisionId!==(prior?.revisionId??null))throw Error('stale_base');
      if(prior&&(prior.payload.kind!==command.payload.kind||Date.parse(prior.sourceObservedAt)>Date.parse(command.sourceObservedAt)||Date.parse(prior.recordedAt)>Date.parse(recordedAt)))throw Error('invalid_revision_transition');
      if(prior?.payload.kind==='teacher_observation'&&command.payload.kind==='teacher_observation'&&canonicalJson(prior.payload.occurrence)!==canonicalJson(command.payload.occurrence))throw Error('observation_identity_changed');
      if(prior){
        for(const field of ['sourceStudent','sourceCourse','sourceBook','sourceUserCourse','sourceMinorUnit','activity','scope','teacherKey'] as const){
          const previous=prior.payload as unknown as Record<string,unknown>,next=command.payload as unknown as Record<string,unknown>;
          if(['source_course_inventory','source_prestudy_snapshot','teacher_preparation_assignment','teacher_activity_revision'].includes(command.payload.kind)&&(field in previous||field in next)&&canonicalJson(previous[field])!==canonicalJson(next[field]))throw Error('preparation_identity_changed');
        }
      }
      let result:WriteAcknowledgement;
      // A fresh teacher observation intent needs its own actor/request/predecessor even when text is identical.
      if(command.payload.kind!=='teacher_observation'&&prior?.contentDigest===contentDigest){
        await tx`UPDATE academy.record_heads SET last_verified_at=${command.sourceObservedAt} WHERE owner_id=${command.ownerId} AND student_id=${command.studentId} AND entity_id=${command.entityId}`;
        result={status:'unchanged',revisionId:prior.revisionId,sequence:prior.sequence,recordedAt:prior.recordedAt,contentDigest,officialSaved:false,parentSent:false};
      }else{
        const body:Omit<RecordVersion,'integrityDigest'>={ownerId:command.ownerId,studentId:command.studentId,entityId:command.entityId,sequence:(prior?.sequence??0)+1,
          revisionId:stableId('revision','academy-v1',command.ownerId,command.actorId,command.idempotencyKey),previousRevisionId:prior?.revisionId??null,previousDigest:prior?.integrityDigest??null,
          payload:command.payload,contentDigest,sourceObservedAt:new Date(command.sourceObservedAt).toISOString(),recordedAt:new Date(recordedAt).toISOString(),actorId:command.actorId,idempotencyKey:command.idempotencyKey,commandDigest};
        const integrityDigest=digest(body);
        await tx`INSERT INTO academy.record_versions(owner_id,student_id,entity_id,sequence,revision_id,previous_revision_id,previous_digest,payload,content_digest,integrity_digest,source_observed_at,recorded_at,actor_id,idempotency_key,command_digest)
          VALUES(${body.ownerId},${body.studentId},${body.entityId},${body.sequence},${body.revisionId},${body.previousRevisionId},${body.previousDigest},${canonicalJson(body.payload)}::text::jsonb,${body.contentDigest},${integrityDigest},${body.sourceObservedAt},${body.recordedAt},${body.actorId},${body.idempotencyKey},${body.commandDigest})`;
        if(catalogRevisionId)await tx`INSERT INTO academy.source_catalog_links(owner_id,student_id,source_revision_id,catalog_revision_id,relation_type) VALUES(${body.ownerId},${body.studentId},${body.revisionId},${catalogRevisionId},'source_book_reference')`;
        await tx`INSERT INTO academy.record_heads(owner_id,student_id,entity_id,revision_id,sequence,content_digest,last_verified_at)
          VALUES(${body.ownerId},${body.studentId},${body.entityId},${body.revisionId},${body.sequence},${contentDigest},${command.sourceObservedAt})
          ON CONFLICT(owner_id,student_id,entity_id) DO UPDATE SET revision_id=EXCLUDED.revision_id,sequence=EXCLUDED.sequence,content_digest=EXCLUDED.content_digest,last_verified_at=EXCLUDED.last_verified_at`;
        result={status:'saved',revisionId:body.revisionId,sequence:body.sequence,recordedAt:body.recordedAt,contentDigest,officialSaved:false,parentSent:false};
      }
      await tx`INSERT INTO academy.command_outcomes(owner_id,actor_id,idempotency_key,command_digest,acknowledgement) VALUES(${command.ownerId},${command.actorId},${command.idempotencyKey},${commandDigest},${canonicalJson(result)}::text::jsonb)`;
      return result;
    });
    const [committed]=await this.db`SELECT * FROM academy.record_versions WHERE owner_id=${command.ownerId} AND student_id=${command.studentId} AND revision_id=${acknowledgement.revisionId}`;
    if(!committed||versionFrom(committed).contentDigest!==acknowledgement.contentDigest)throw Error('committed_outcome_uncertain');
    return acknowledgement;
  }
  async observationResult(ownerId:string,studentId:string,actorId:string,requestId:string):Promise<RecordVersion|null>{
    const rows=await this.db`SELECT v.* FROM academy.command_outcomes o JOIN academy.record_versions v
      ON v.owner_id=o.owner_id AND v.actor_id=o.actor_id AND v.revision_id=o.acknowledgement->>'revisionId'
      WHERE o.owner_id=${ownerId} AND o.actor_id=${actorId} AND o.idempotency_key=${requestId} AND v.student_id=${studentId}`;
    if(rows.length!==1)return null;
    const record=versionFrom(rows[0]);return record.payload.kind==='teacher_observation'?record:null;
  }
  async history(ownerId:string,studentId:string,entityId:string,afterSequence=0,limit=200):Promise<RecordVersion[]> {
    if(!Number.isSafeInteger(afterSequence)||afterSequence<0||!Number.isSafeInteger(limit)||limit<1||limit>1000)throw Error('invalid_history_range');
    const rows=await this.db`SELECT * FROM academy.record_versions WHERE owner_id=${ownerId} AND student_id=${studentId} AND entity_id=${entityId} AND sequence>${afterSequence} ORDER BY sequence LIMIT ${limit}`;
    return rows.map((row:VersionRow)=>versionFrom(row));
  }
  async current(ownerId:string,studentId:string):Promise<(RecordVersion&{lastVerifiedAt:string})[]> {
    const rows=await this.db`SELECT v.*,h.last_verified_at FROM academy.record_heads h JOIN academy.record_versions v USING(owner_id,student_id,entity_id,sequence) WHERE h.owner_id=${ownerId} AND h.student_id=${studentId} ORDER BY h.entity_id`;
    return rows.map((row:VersionRow&{last_verified_at:Date|string})=>({...versionFrom(row),lastVerifiedAt:new Date(row.last_verified_at).toISOString()}));
  }
  async preparationRevisions(ownerId:string,studentId:string,date:string):Promise<RecordVersion[]> {
    requireText(ownerId,'owner');requireText(studentId,'student');assertPreparationDate(date);
    const rows=await this.db`SELECT * FROM academy.record_versions WHERE owner_id=${ownerId} AND student_id=${studentId}
      AND payload->>'kind' IN ('teacher_activity_revision','teacher_preparation_assignment') AND payload->>'effectiveDate'<=${date}
      ORDER BY entity_id,sequence`;
    return rows.map((row:VersionRow)=>versionFrom(row));
  }
  async saveCatalog(ownerId:string,node:CatalogNode):Promise<{revisionId:string;sequence:number;contentDigest:string}> {
    requireText(ownerId,'catalog owner');canonicalJson(node);
    return this.db.begin(async tx=>{
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${canonicalJson([ownerId,'catalog-write'])},0))`;
      const rows=await tx`SELECT DISTINCT ON(node_id) node_id,payload,sequence,revision_id,content_digest FROM academy.catalog_versions WHERE owner_id=${ownerId} ORDER BY node_id,sequence DESC`;
      const nodes=rows.map((row:{payload:CatalogNode})=>row.payload).filter((existing:CatalogNode)=>existing.id!==node.id);
      validateHierarchy([...nodes,node]);
      if(nodes.some((existing:CatalogNode)=>canonicalJson(existing.sourceKey)===canonicalJson(node.sourceKey)))throw Error('duplicate_catalog_source_identity');
      const prior=rows.find((row:{node_id:string})=>row.node_id===node.id);
      if(prior&&(prior.payload.kind!==node.kind||canonicalJson(prior.payload.sourceKey)!==canonicalJson(node.sourceKey)))throw Error('catalog_identity_changed');
      const contentDigest=digest(node);
      if(prior?.content_digest===contentDigest)return {revisionId:prior.revision_id,sequence:Number(prior.sequence),contentDigest};
      const sequence=Number(prior?.sequence??0)+1;
      const revisionId=stableId('catalog_revision','academy-v1',ownerId,node.id,String(sequence),contentDigest);
      await tx`INSERT INTO academy.catalog_versions(owner_id,node_id,sequence,revision_id,payload,content_digest) VALUES(${ownerId},${node.id},${sequence},${revisionId},${canonicalJson(node)}::text::jsonb,${contentDigest})`;
      return {revisionId,sequence,contentDigest};
    });
  }
  async catalog(ownerId:string):Promise<CatalogNode[]> {
    const rows=await this.db`SELECT DISTINCT ON(node_id) payload,content_digest FROM academy.catalog_versions WHERE owner_id=${ownerId} ORDER BY node_id,sequence DESC`;
    const nodes=rows.map((row:{payload:CatalogNode;content_digest:string})=>{if(digest(row.payload)!==row.content_digest)throw Error('catalog_integrity');return row.payload;});
    validateHierarchy(nodes);return nodes;
  }
}
