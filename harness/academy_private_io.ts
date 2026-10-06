import {constants,openSync,closeSync,fstatSync,readFileSync,realpathSync,writeSync,ftruncateSync,fsyncSync} from 'node:fs';
import {dirname,relative,resolve,isAbsolute,basename} from 'node:path';
export function privateJsonPath(home:string,path:string){
 const target=resolve(path),parent=realpathSync(dirname(target));
 if(parent!==dirname(target)||!target.endsWith('.json')||/^(auth|config|credentials)\./i.test(basename(target)))throw Error('private_json_path_required');
 if(!['artifacts','cache/scratch'].some(folder=>{const rel=relative(resolve(home,folder),parent);return rel===''||(!rel.startsWith('..')&&!isAbsolute(rel));}))throw Error('private_json_path_required');
 return target;
}
function checkDescriptor(fd:number,target:string){
 const stat=fstatSync(fd);let actual:string;try{actual=realpathSync('/proc/self/fd/'+fd);}catch{throw Error('private_json_target_changed');}
 if(!stat.isFile()||stat.nlink!==1||actual!==target||(stat.mode&0o077)!==0||stat.uid!==process.getuid?.())throw Error('private_json_target_changed');
 return stat;
}
export function readPrivateJsonText(home:string,path:string){
 const target=privateJsonPath(home,path),fd=openSync(target,constants.O_RDONLY|constants.O_NOFOLLOW);
 try{const stat=checkDescriptor(fd,target);if(stat.size>131072)throw Error('private_json_input_too_large');const text=readFileSync(fd,'utf8');checkDescriptor(fd,target);return text;}finally{closeSync(fd);}
}
/** Keep the exclusive descriptor: no reopening/truncating an attacker-replaced pathname. */
export function reservePrivateJson(home:string,path:string){
 const target=privateJsonPath(home,path),fd=openSync(target,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o600);let closed=false;
 const close=()=>{if(!closed){closed=true;closeSync(fd);}};
 try{checkDescriptor(fd,target);}catch(error){close();throw error;}
 return {path:target,write:(value:unknown)=>{
   if(closed)throw Error('private_json_descriptor_closed');checkDescriptor(fd,target);
   const buffer=Buffer.from(JSON.stringify(value,null,2)+'\n');let offset=0;
   while(offset<buffer.length)offset+=writeSync(fd,buffer,offset,buffer.length-offset,offset);
   ftruncateSync(fd,buffer.length);fsyncSync(fd);checkDescriptor(fd,target);
 },close};
}
