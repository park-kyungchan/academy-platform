import { describe, expect, spyOn, test } from 'bun:test';
import { nativeBookAcquirer } from '../../src/academy/nativeConnector';
describe('native source connector scope',()=>{
  test('refuses empty source scope before native transport or credential use',async()=>{
    await expect(nativeBookAcquirer({teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001',studentNames:[]})).rejects.toThrow('invalid_source_scope');
  });
  test('a valid request blocks unqualified direct authentication without launching the retired browser bridge',async()=>{
    const prior=process.env.HERMES_HOME;
    process.env.HERMES_HOME='/opt/data/profiles/dc-gang-a-dev';
    const spawn=spyOn(Bun,'spawn').mockImplementation(()=>{throw Error('unexpected_native_process');});
    try{
      const result=await nativeBookAcquirer({teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001',studentNames:['Invented Learner']});
      expect(result).toEqual({status:'blocked',reason:'direct_http_authentication_not_qualified'});
      expect(spawn).not.toHaveBeenCalled();
    }finally{
      spawn.mockRestore();
      if(prior===undefined)delete process.env.HERMES_HOME;else process.env.HERMES_HOME=prior;
    }
  });
});
