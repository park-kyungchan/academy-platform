import {expect,test} from 'bun:test';
import {z} from 'zod';
import {DailyRecordService,dailyRecordReconcileSchema,dailyRecordIntakeSchema,dailyRecordReviewSchema,dailyRecordSaveSchema,studentMetadataBatchSchema,studentMetadataGetSchema} from '../src/dailyrecord.ts';
import {dailyTestHeaderRouteSchema,readDailyTestHeaders} from '../src/dailytest-header-route.ts';
import {dailyRecordGetSchema} from '../src/dailyrecord-read.ts';

// 2040-03-15 12:00 in Asia/Seoul, a fixed clock.
const clock=Date.parse('2040-03-15T03:00:00Z'),today='2040-03-15';
const base={purpose:'Synthetic read',currentRequestRef:'synthetic-request'};
const name='Fictional Student';
const queries=[{kind:'daily_inventory' as const}];
const scope={sourceNamespace:'academy.lms.metadata' as const,teacherId:'99001',studentId:'77001'};

test('a metadata get without lessonDate reads today in Asia/Seoul; a given date resolves as before',async()=>{
  expect(studentMetadataGetSchema.safeParse({displayName:name,queries}).success).toBe(true);
  const seen:string[]=[];
  const backend={readStudentMetadataByName:async(input:any)=>{seen.push(input.lessonDate);return {scope:{...scope,lessonDate:input.lessonDate},results:input.queries.map((query:unknown)=>({query,observation:{}}))};}};
  const service=new DailyRecordService(backend as never,null,()=>clock);
  expect(await service.metadataByName({...base,displayName:name,queries})).toMatchObject({status:'observed',lessonDate:today});
  for(const [given,resolved] of [['today',today],['오늘',today],['yesterday','2040-03-14'],['어제','2040-03-14'],['3월 10일','2040-03-10'],['3/10','2040-03-10'],['2040-03-01','2040-03-01']] as const){
    await service.metadataByName({...base,displayName:name,lessonDate:given,queries});
    expect(seen.at(-1)).toBe(resolved);
  }
});

test('a metadata batch without scope.lessonDate reads today in Asia/Seoul',async()=>{
  expect(studentMetadataBatchSchema.safeParse({...base,scope,queries}).success).toBe(true);
  const seen:string[]=[];
  const backend={readStudentMetadata:async(input:any)=>{seen.push(input.scope.lessonDate);return {scope:input.scope,results:input.queries.map((query:unknown)=>({query,observation:{}}))};}};
  const service=new DailyRecordService(backend as never,null,()=>clock);
  expect(await service.metadata({...base,scope,queries})).toMatchObject({status:'observed',scope:{lessonDate:today}});
  await service.metadata({...base,scope:{...scope,lessonDate:'어제'},queries});
  expect(seen).toEqual([today,'2040-03-14']);
});

test('the DailyTest header route without lessonDate reads today in Asia/Seoul',async()=>{
  const request={displayName:name,...base,periods:['11']};
  expect(dailyTestHeaderRouteSchema.safeParse(request).success).toBe(true);
  const seen:string[]=[];
  const port={metadata:async(input:any)=>{seen.push(input.lessonDate);throw Error('stop_after_date');},headers:async()=>{throw Error('must_not_call');}};
  await expect(readDailyTestHeaders(request,port,()=>clock)).rejects.toThrow('stop_after_date');
  await expect(readDailyTestHeaders({...request,lessonDate:'3/10'},port,()=>clock)).rejects.toThrow('stop_after_date');
  expect(seen).toEqual([today,'2040-03-10']);
});

test('strict schemas stay strict and a bad date is still refused',()=>{
  expect(studentMetadataGetSchema.safeParse({displayName:name,queries,extra:1}).success).toBe(false);
  expect(studentMetadataBatchSchema.safeParse({...base,scope:{...scope,extra:1},queries}).success).toBe(false);
  expect(studentMetadataBatchSchema.safeParse({...base,scope,queries,extra:1}).success).toBe(false);
  expect(dailyTestHeaderRouteSchema.safeParse({displayName:name,periods:['11'],extra:1}).success).toBe(false);
  expect(dailyTestHeaderRouteSchema.safeParse({displayName:name,periods:['11'],lessonDate:''}).success).toBe(false);
  expect(studentMetadataGetSchema.safeParse({displayName:name,queries}).data?.lessonDate).toBe('today');
});

test('the input schemas advertise lessonDate as optional on the three read tools',()=>{
  for(const schema of [studentMetadataGetSchema,studentMetadataBatchSchema.shape.scope,dailyTestHeaderRouteSchema,dailyRecordGetSchema]){
    const json=z.toJSONSchema(schema as never,{io:'input'}) as {required?:string[];properties:Record<string,{description?:string}>};
    expect(json.required??[]).not.toContain('lessonDate');
    expect(json.properties.lessonDate!.description).toContain('Omit for today');
  }
});

test('the memo bound of intake, review and save is stated in the input schema',()=>{
  for(const schema of [dailyRecordIntakeSchema,dailyRecordReviewSchema,dailyRecordSaveSchema]){
    const text=JSON.stringify(z.toJSONSchema(schema as never,{io:'input'}));
    expect(text).toContain('at most 400 units');
    expect(text).toContain('non-ASCII character');
    expect(text).toContain('counts 2');
  }
});

test('the memo content member stays strict and its bound is not stated on the context of other tools',()=>{
  const memo=(extra:object)=>dailyRecordIntakeSchema.safeParse({context:{scope:{sourceNamespace:'academy.lms.dayrecord',teacherId:'99001',studentId:'77001',lessonDate:'2040-03-15'},memoAnswer:{kind:'content',requestRef:'r',text:'메모',...extra}}});
  expect(memo({}).success).toBe(true);
  expect(memo({extra:1}).success).toBe(false);
  expect(JSON.stringify(z.toJSONSchema(dailyRecordReconcileSchema as never,{io:'input'}))).not.toContain('at most 400 units');
});
