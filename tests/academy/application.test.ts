import { describe, expect, test } from 'bun:test';
import { SQL } from 'bun';
import { TeacherApplication } from '../../src/academy/application';
import { PostgresAcademyStore } from '../../src/academy/postgresStore';
const socket=process.env.DC_ACADEMY_TEST_SOCKET;
describe.skipIf(!socket)('shared teacher/application query boundary',()=>{
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
