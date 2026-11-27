import {execFile} from 'node:child_process';
import {isAbsolute} from 'node:path';
import {academyBusinessDate} from './read-input.ts';

/** One active binding of a student in the `edu-academy-memory` store: the keys of a book the
 * live getTextBookProgress list may no longer hold (specs/academy-mcp/direct-key-lookup.md R2).
 * The values are the store's own text; the backend checks them before any LMS request. */
export interface HistoricalBinding {bookNo:string;courseKey:string;bookLabel:string}
/** Read-only source of a student's active bindings (R7): it never binds, unbinds or writes. */
export interface HistoricalBindingReader {
  /** The active bindings of the one store student whose `name` is `displayName` and whose
   * `student_id`, when the store holds one, is `studentId`; [] when no such student. */
  activeBindings(displayName:string,studentId:string):Promise<HistoricalBinding[]>;
}

/** The subcommands this reader may run: reads only. `bind`, `unbind`, `observed`, `note`,
 * `remind` and every other write are refused before a process starts. */
export const memoryReadSubcommands=new Set(['roster','show','due']);
const MAX_ROSTER=256;
const slugPattern=/^[A-Za-z0-9._-]{1,32}$/;
export type MemoryRun=(args:readonly string[])=>Promise<string>;

/** `edu-academy-memory` as a process: `--store <storePath>` always precedes the subcommand, and
 * the environment is fixed, so no read depends on HOME or the caller's environment. */
export function memoryProcess(memoryBin:string,storePath:string,timeoutMs=15000):MemoryRun {
  if(!isAbsolute(memoryBin)||!isAbsolute(storePath))throw Error('memory_bindings_arguments_invalid');
  return args=>{
    if(!memoryReadSubcommands.has(args[0]!))return Promise.reject(Error('historical_binding_reader_refused'));
    return new Promise((resolve,reject)=>execFile(memoryBin,['--store',storePath,...args],{encoding:'utf8',timeout:timeoutMs,maxBuffer:1024*1024,
      env:{PATH:'/usr/local/bin:/usr/bin:/bin',LANG:'C.UTF-8',EDU_ONT_BIN:'/usr/local/bin/ont'}},
      (error,stdout)=>error?reject(Error('historical_binding_reader_failed')):resolve(stdout)));
  };
}

/** The production reader over the `edu-academy-memory` reads of crates/edu-academy-memory/README.md:
 * `roster` (at most 256 slugs), `show student <slug>` (the `name:` and `student_id:` lines) and
 * `due <slug> <today>` (its complete JSON `bindings`). No error holds a name, slug or key. */
export class EduAcademyMemoryBindingReader implements HistoricalBindingReader {
  constructor(readonly run:MemoryRun,readonly today:()=>string=()=>academyBusinessDate(Date.now())){}
  async activeBindings(displayName:string,studentId:string):Promise<HistoricalBinding[]>{
    const slugs=(await this.run(['roster'])).split('\n').map(line=>line.split('\t')[0]!.trim()).filter(Boolean);
    if(slugs.length>MAX_ROSTER||slugs.some(slug=>!slugPattern.test(slug)))throw Error('historical_binding_reader_failed');
    const matched:string[]=[];
    for(const slug of slugs){
      const properties=new Map<string,string>();
      for(const line of (await this.run(['show','student',slug])).split('\n')){
        const at=line.indexOf(': ');
        if(at>0&&!properties.has(line.slice(0,at)))properties.set(line.slice(0,at),line.slice(at+2));
      }
      if(properties.get('name')!==displayName)continue;
      const stored=properties.get('student_id');
      if(stored!==undefined&&stored!==''&&stored!=='-'&&stored!==studentId)continue;
      matched.push(slug);
    }
    if(matched.length>1)throw Error('historical_binding_ambiguous');
    if(!matched.length)return [];
    let due:unknown;
    try{due=JSON.parse(await this.run(['due',matched[0]!,this.today()]));}catch{throw Error('historical_binding_reader_failed');}
    const bindings=(due as {bindings?:unknown})?.bindings;
    if(!Array.isArray(bindings)||bindings.length>256)throw Error('historical_binding_reader_failed');
    return bindings.map(raw=>{
      const b=(raw??{}) as Record<string,unknown>;
      if(typeof b.book_no!=='string'&&typeof b.book_no!=='number')throw Error('historical_binding_reader_failed');
      return {bookNo:String(b.book_no),courseKey:b.course_key==null?'':String(b.course_key),bookLabel:typeof b.book_label==='string'?b.book_label:''};
    });
  }
}

/** `--memory-bindings <memoryBin> <storePath>`: both paths explicit. Without it the fallback
 * takes explicit parameters only. */
export function historicalBindingReaderConfiguration(args:readonly string[]){
  const remainingArgs:string[]=[];let reader:HistoricalBindingReader|null=null;
  for(let i=0;i<args.length;i++){
    if(args[i]!=='--memory-bindings'){remainingArgs.push(args[i]!);continue;}
    if(reader||i+2>=args.length)throw Error('memory_bindings_arguments_invalid');
    reader=new EduAcademyMemoryBindingReader(memoryProcess(args[i+1]!,args[i+2]!));
    i+=2;
  }
  return {remainingArgs,reader};
}
