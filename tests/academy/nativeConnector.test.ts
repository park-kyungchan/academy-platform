import { describe, expect, test } from 'bun:test';
import { nativeBookAcquirer } from '../../src/academy/nativeConnector';
describe('native source connector scope',()=>{
  test('refuses empty source scope before native transport or credential use',async()=>{
    await expect(nativeBookAcquirer({teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001',studentNames:[]})).rejects.toThrow('invalid_source_scope');
  });
});
