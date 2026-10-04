import {lstatSync,readFileSync,realpathSync,writeFileSync} from 'node:fs';
import {basename,dirname,isAbsolute,relative,resolve} from 'node:path';
import {createAcademyReadPlan} from '../src/lms/academyReadProtocol';
import {createAcademySourceAudit} from '../src/lms/academySourceAudit';

function privatePath(path:string,home:string,extension:string,existing:boolean):string {
  if(!path||!home||!path.endsWith(extension)||/^(?:auth|credentials|config)\./i.test(basename(path)))throw new Error('private_read_path_required');
  const root=realpathSync(home),target=resolve(path),parent=realpathSync(dirname(target));
  if(dirname(target)!==parent)throw new Error('private_read_path_required');
  const allowed=[resolve(root,'cache/scratch'),resolve(root,'artifacts')];
  if(!allowed.some(base=>{const rel=relative(base,parent);return rel===''||(!rel.startsWith('..')&&!isAbsolute(rel));}))throw new Error('private_read_path_required');
  if(existing){const stat=lstatSync(target);if(!stat.isFile()||stat.isSymbolicLink()||stat.nlink!==1||stat.size>65536)throw new Error('invalid_private_read_input');}
  return target;
}
/** Offline compilation and an exclusive owner-only file. No HTTP or secret acquisition. */
export async function prepareAcademyReadFile(inputPath:string,outputPath:string,home=process.env.HERMES_HOME??'') {
  const input=privatePath(inputPath,home,'.json',true),output=privatePath(outputPath,home,'.js',false);
  const request=JSON.parse(readFileSync(input,'utf8'));
  const plan=createAcademyReadPlan(request);
  // No current academy header contract is installed. Do not compile a guessed collector.
  if(plan.family==='day_record')throw new Error('day_record_column_contract_required');
  const built=await Bun.build({entrypoints:[resolve(import.meta.dir,'native_read_entry.ts')],target:'browser',format:'esm',minify:false});
  if(!built.success||built.outputs.length!==1)throw new Error('native_read_bundle_failed');
  const code=await built.outputs[0]!.text(),boundary=code.lastIndexOf('\nexport {');
  if(boundary<0||!code.slice(boundary).includes('runNativeAcademyRead'))throw new Error('native_read_bundle_exports_invalid');
  const expression='(()=>{'+code.slice(0,boundary)+'\nreturn runNativeAcademyRead('+JSON.stringify(plan.request)+');})()';
  if(Buffer.byteLength(expression)>131072)throw new Error('native_read_bundle_too_large');
  writeFileSync(privatePath(output,home,'.js',false),expression,{encoding:'utf8',flag:'wx',mode:0o600});
  return {schemaVersion:1,prepared:true,family:plan.family,networkRequests:0,credentialValuesWritten:false,
    outputScript:output,nativeAuthentication:'browser_vault_required',effect:'read_only',recordsRequested:plan.family==='textbook'?plan.request.sourceTitles.length:plan.family==='daily_zero'?plan.request.targets.length:1,
    privateScopeValuesEmitted:false,automaticRetryAllowed:false};
}

export function auditAcademyReadFile(inputPath:string,outputPath:string,home=process.env.HERMES_HOME??'') {
  const input=privatePath(inputPath,home,'.json',true),output=privatePath(outputPath,home,'.json',false);
  const audit=createAcademySourceAudit(JSON.parse(readFileSync(input,'utf8')));
  writeFileSync(privatePath(output,home,'.json',false),JSON.stringify(audit,null,2)+'\n',{encoding:'utf8',flag:'wx',mode:0o600});
  return {schemaVersion:1,audited:true,sourceFamily:audit.content.sourceNamespace.replace('academy_',''),contentDigest:audit.contentDigest,
    outputAudit:output,networkRequests:0,privateSourceValuesEmitted:false,productionWriteAuthorized:false};
}
