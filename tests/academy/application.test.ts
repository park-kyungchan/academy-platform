import { describe, expect, test } from 'bun:test';
import { SQL } from 'bun';
import { TeacherApplication } from '../../src/academy/application';
import { PostgresAcademyStore } from '../../src/academy/postgresStore';
import { randomUUID } from 'node:crypto';
const socket=process.env.DC_ACADEMY_TEST_SOCKET;
describe.skipIf(!socket)('shared teacher/application query boundary',()=>{
  test('serves a date-first official-source scene without hiding another scheduled student or inventing preparation joins',async()=>{
    const db=new SQL({adapter:'postgres',path:socket!,username:'hermes',database:'dc_academy_test'});
    const store=new PostgresAcademyStore(db);await store.migrate();
    const owner='fixture-date-owner-'+randomUUID();
    for(const [index,name] of ['Invented Learner A','Invented Learner B'].entries()){
      const studentKey=String(91001+index),id='fixture-date-student-'+index;
      await store.registerStudent({ownerId:owner,id,displayName:name,sourceKey:{namespace:'academy_student',literalKey:studentKey},proofRefs:['fixture']});
      const acquisition={schemaVersion:1,family:'day_record',status:'observed',records:[],dailyResults:[],dayRecords:[{studentName:name,date:'2090-01-02',groupId:'21001',teacherKey:'11001',studentKey,courseKey:'99001',recordKey:String(41001+index),membershipKey:String(81001+index),fields:{attendance:index?'N':'Y',progress:'Invented progress',homework:'Invented range'}}],requestedScope:{family:'day_record',teacherLabel:'Invented Teacher',date:'2090-01-02',groupIds:['21001'],studentNames:[name]},observedAt:'2090-01-02T00:00:00Z',coverage:'requested_date_groups_students_not_cohort_completeness',unknownStates:['app_join_unknown'],nativePrintInvoked:false,automaticRetryAllowed:false};
      await store.append({ownerId:owner,studentId:id,entityId:'fixture-lesson-'+index,actorId:'fixture-source-reader',idempotencyKey:'fixture-read-'+index,expectedRevisionId:null,sourceObservedAt:acquisition.observedAt,payload:{kind:'source_audit',acquisition,contractRef:'fixture'}},'2090-01-02T00:00:01Z');
    }
    const app=new TeacherApplication(store,owner),context={ownerId:owner,actorId:'fixture-teacher',role:'teacher' as const};
    const result=await app.classPreparation(context,'2090-01-02');
    expect((await app.cohort(context)).availableLessonDates).toEqual(['2090-01-02']);
    expect(result.rows).toHaveLength(2);
    expect(result.rows.some(row=>row.fields.attendance==='N')).toBe(true);
    expect(result.rows.every(row=>row.knowledge.homeworkAttemptJoin==='unknown')).toBe(true);
    expect((await app.classPreparation(context,'2090-01-03')).rows).toHaveLength(0);
    await expect(app.classPreparation({...context,ownerId:'foreign'},'2090-01-02')).rejects.toThrow('owner_scope_conflict');
    await db.close();
  });
  test('serves the same bounded source-grounded scene and refuses caller-owned foreign partitions',async()=>{
    const db=new SQL({adapter:'postgres',path:socket!,username:'hermes',database:'dc_academy_test'});
    const store=new PostgresAcademyStore(db);
    const app=new TeacherApplication(store,'fixture-owner-a');
    const scene=await app.student({ownerId:'fixture-owner-a',actorId:'fixture-user',role:'teacher'},'fixture-student-a');
    expect(scene.student.displayName).toBe('Invented Learner');
    expect(scene.knowledge.mastery).toBe('not_established');
    await expect(app.student({ownerId:'fixture-owner-b',actorId:'foreign-user',role:'teacher'},'fixture-student-a')).rejects.toThrow('owner_scope_conflict');
    await db.close();
  });
});
