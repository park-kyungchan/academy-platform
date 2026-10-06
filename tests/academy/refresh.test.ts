import { beforeAll, describe, expect, test } from 'bun:test';
import {ensureSyntheticStudentFixture} from './postgresFixture';
import { SourceRefreshCoordinator } from '../../src/academy/refresh';
import { SQL } from 'bun';
import { PostgresAcademyStore } from '../../src/academy/postgresStore';
const socket=process.env.DC_ACADEMY_TEST_SOCKET;
describe.skipIf(!socket)('source refresh failure recovery',()=>{
  beforeAll(ensureSyntheticStudentFixture);
  test('authentication failure leaves previous verified source heads untouched',async()=>{
    const db=new SQL({adapter:'postgres',path:socket!,username:'hermes',database:'dc_academy_test'});
    const store=new PostgresAcademyStore(db);
    const before=await store.current('fixture-owner-a','fixture-student-a');
    const coordinator=new SourceRefreshCoordinator(store,{ownerId:'fixture-owner-a',teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001'},async()=>({status:'blocked',reason:'authentication_required'}));
    const result=await coordinator.refresh({ownerId:'fixture-owner-a',actorId:'fixture-user',role:'teacher'},'fixture-student-a');
    expect(result.status).toBe('blocked');expect(result.cacheRetained).toBe(true);
    expect(await store.current('fixture-owner-a','fixture-student-a')).toEqual(before);
    await db.close();
  });
});
