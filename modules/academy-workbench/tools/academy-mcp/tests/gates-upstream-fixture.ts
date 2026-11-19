import './network-deny.ts';
import {appendFileSync,readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {McpServer} from '@modelcontextprotocol/server';
import {StdioServerTransport} from '@modelcontextprotocol/server/stdio';
import {registerDailyRecordTools,dailyRecordToolConfiguration} from '../src/dailyrecord-tools.ts';
import {DailyRecordService} from '../src/dailyrecord.ts';
import {NativeDailyRecordBackend} from '../src/native-dailyrecord-backend.ts';

// A fictional dailyrecord endpoint launched as an installed one is: with the old
// `--dailyrecord-admission` and `--metadata-admission` arguments. Every tool call
// is appended to the events file by name. No network, no LMS. The unlisted
// student's textbook list (`fixtures/textbook-list.json`) has two books of one series and one
// of another, as a lesson session meets them.
const events=process.argv[2]!;
const config=await dailyRecordToolConfiguration(process.argv.slice(3));
const legacy=config as unknown as {ownerAdmission?:unknown;metadataAdmission?:unknown};
const previousDate=(date:string)=>new Date(Date.parse(date+'T00:00:00Z')-86400000).toISOString().slice(0,10);
const keyOf=(name:string)=>name==='Unlisted Student'?'77009':name==='Invented Student'?'77001':null;
const row=(name:string,key:string,date:string)=>({studentName:name,studentKey:key,teacherKey:'99001',date,groupId:'3',courseKey:'5',recordKey:date.replaceAll('-',''),membershipKey:'8',
  fields:{progress:'- Fictional progress',homework:'- Fictional homework',lessonMemo:'',attendance:'Y'}});
const textbookList=(JSON.parse(readFileSync(resolve(import.meta.dir,'fixtures/textbook-list.json'),'utf8')) as {bindings:{studentKey:string}[]}).bindings;
const port={authenticate:async()=>{},discoverDayRecordGroups:async()=>({groups:[{groupId:'3'}]}),
  searchStudentsByName:async(input:any)=>{const key=keyOf(input.studentName);return key?[{studentKey:key,studentName:input.studentName}]:[];},
  execute:async(input:any)=>{const name=input.studentNames[0],key=keyOf(name);return {dayRecords:key?[row(name,key,input.date)]:[]};},
  journalState:async(target:any)=>({target,fields:{...row(target.studentName,target.studentKey,target.date).fields,dailyTestAssessment:'0',previousHomeworkAssessment:'3'},
    previousRecordKey:previousDate(target.date).replaceAll('-',''),previousDate:previousDate(target.date),wireDigest:'a'.repeat(64)}),
  readTextbookBindings:async(input:any)=>textbookList.filter(b=>b.studentKey===input.studentKey),
  inspectTextbookIndex:async(input:any)=>({rows:[{sourceTitle:'Section of book '+input.bookNo,sourceDate:{literal:'11-10',meaning:'source_apply_date_not_exact_grade_time'}}]}),
  close:()=>{}};
const launch={teacherId:'99001',teacherLabel:'Invented Teacher',date:'2040-01-16',students:[{studentId:'77001',displayName:'Invented Student'}],sourceBundleSha256:'b'.repeat(64)};
const backend=new NativeDailyRecordBackend(port as never,launch,()=>({username:'fictional',password:'fictional'}),
  (legacy.ownerAdmission??null) as never,new Set(['Invented Student']) as never,(legacy.metadataAdmission??null) as never);
const server=new McpServer({name:'fictional-gates-upstream',version:'1.0.0'});
const register=server.registerTool.bind(server);
(server as any).registerTool=(name:string,definition:unknown,handler:(...args:any[])=>unknown)=>
  register(name,definition as never,(async (...args:any[])=>{appendFileSync(events,'call:'+name+'\n');return handler(...args);}) as never);
registerDailyRecordTools(server,new DailyRecordService(backend));
await server.connect(new StdioServerTransport());
