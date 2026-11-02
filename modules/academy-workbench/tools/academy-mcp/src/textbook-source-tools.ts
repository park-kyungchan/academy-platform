import {createHash} from 'node:crypto';
import {mkdir,lstat,readFile,writeFile,realpath} from 'node:fs/promises';
import {resolve} from 'node:path';
import {z} from 'zod';
import type {McpServer} from '@modelcontextprotocol/server';
import type {TextbookCatalogObservation,TextbookPdfObservation,TextbookSchoolLevel} from '../../../../../apps/academy/dc-gang-a/src/lms/textbookCatalogRead';
import {publicReadFailure} from './read-failure.ts';
import {toolResult} from './tool-result.ts';
import {readOnlyAnnotations} from './contracts.ts';
import {academyRoutingDescription} from './routing-context.ts';

export const textbookCatalogInput=z.object({schoolLevel:z.enum(['elementary','middle','high']).default('middle'),documentKind:z.enum(['sample','answer','repair','daily','other','all']).default('all'),query:z.string().trim().min(1).max(200).optional(),offset:z.number().int().min(0).max(1000).default(0),limit:z.number().int().min(1).max(100).default(10)}).strict();
export const textbookPdfInput=z.object({schoolLevel:z.enum(['elementary','middle','high']).default('middle'),documentRef:z.string().regex(/^[a-f0-9]{64}$/)}).strict();
export interface TextbookSourceBackend {
  readTextbookCatalog(schoolLevel:TextbookSchoolLevel):Promise<TextbookCatalogObservation>;
  readTextbookPdf(schoolLevel:TextbookSchoolLevel,documentRef:string):Promise<TextbookPdfObservation>;
}
/** Content-addressed original bytes, private to the verified owner launch. */
export async function preserveTextbookPdf(root:string,observation:TextbookPdfObservation){
  const bytes=observation.bytes,sha256=createHash('sha256').update(bytes).digest('hex');
  if(sha256!==observation.sha256||bytes.byteLength!==observation.byteLength)throw Error('textbook_pdf_digest_rejected');
  await mkdir(root,{recursive:true,mode:0o700});
  if(await realpath(root)!==resolve(root)||(await lstat(root)).isSymbolicLink())throw Error('textbook_artifact_directory_rejected');
  const path=resolve(root,sha256+'.pdf');
  try{await writeFile(path,bytes,{mode:0o600,flag:'wx'});}
  catch(error){if((error as NodeJS.ErrnoException).code!=='EEXIST')throw error;}
  const stat=await lstat(path);
  if(!stat.isFile()||stat.isSymbolicLink()||stat.size!==bytes.byteLength||createHash('sha256').update(await readFile(path)).digest('hex')!==sha256)
    throw Error('textbook_artifact_readback_rejected');
  return {path,sha256,bytes:stat.size,custody:'native_owner_filesystem' as const};
}
const limits={studentBinding:'not_established',exactPhysicalEdition:'not_established',completion:'not_established',academyWrites:0,parentSends:0,prints:0};
const publicCodes=new Set([
  'textbook_catalog_principal_rejected','textbook_catalog_school_level_rejected','textbook_catalog_authentication_required',
  'textbook_catalog_row_unqualified','textbook_catalog_pdf_links_unqualified','textbook_catalog_source_port_unavailable',
  'textbook_pdf_link_rejected','textbook_pdf_input_rejected','textbook_pdf_not_in_current_catalog','textbook_pdf_transport_failed',
  'textbook_pdf_response_url_rejected','textbook_pdf_signature_rejected','textbook_pdf_source_port_unavailable',
  'textbook_pdf_digest_rejected','textbook_pdf_identity_rejected','textbook_artifact_directory_rejected','textbook_artifact_readback_rejected',
  ...['catalog','pdf'].flatMap(kind=>['response_rejected','content_type_rejected','size_rejected','body_missing'].map(reason=>'textbook_'+kind+'_'+reason)),
]);
function failure(error:unknown){
  const message=error instanceof Error?error.message:'';
  const code=publicCodes.has(message)?message:['authentication_required','source_authentication_failed'].includes(message)?'textbook_authentication_failed':'textbook_source_read_failed';
  const status=(error as {httpStatus?:unknown}|null)?.httpStatus;
  const sourceHttpStatus=typeof status==='number'&&Number.isInteger(status)&&status>=400&&status<=599?status:null;
  return toolResult({status:'failed',code,failure:publicReadFailure(error,'textbook_source_read_failed'),sourceHttpStatus,automaticRetryAllowed:false,...limits});
}
export function registerTextbookSourceTools(server:McpServer,backend:TextbookSourceBackend|null,artifactRoot:string){
  server.registerTool('academy_textbook_catalog',{
    title:'Read current textbook and workbook PDF catalog',
    description:academyRoutingDescription('academy_textbook_catalog','Read the authenticated elementary/middle/high textbook and workbook PDF catalog (limit1–100, default10). Filter actual row labels/filenames and documentKind: sample, answer, repair, daily or all. Preserves source rowspans. Gauss Plus workbook may expose an answer PDF rather than a sample. Returns exact documentRefs; does not infer physical editions, student binding, assignments or completion.'),
    inputSchema:textbookCatalogInput,annotations:{...readOnlyAnnotations,openWorldHint:true},
  },async input=>{
    if(!backend)return toolResult({status:'unavailable',code:'textbook_source_backend_not_configured',...limits});
    try{const full=await backend.readTextbookCatalog(input.schoolLevel);
      const entries=full.entries.filter(e=>(input.documentKind==='all'||e.kind===input.documentKind)&&(!input.query||[e.filename,...e.sourceLabels].some(s=>s.toLocaleLowerCase().includes(input.query!.toLocaleLowerCase()))));
      const selected=entries.slice(input.offset,input.offset+input.limit),end=input.offset+selected.length;
      return toolResult({...full,entries:selected,page:{totalObservedPdfLinks:full.entries.length,totalMatched:entries.length,returned:selected.length,nextOffset:end<entries.length?end:null},...limits});
    }catch(error){return failure(error);}
  });
  server.registerTool('academy_textbook_pdf',{
    title:'Download and preserve one observed textbook PDF',
    description:academyRoutingDescription('academy_textbook_pdf','For the exact schoolLevel/documentRef from academy_textbook_catalog, re-read that catalog, verify its actual storage link and download bounded original PDF bytes without forwarding academy credentials. Supports observed sample and answer/workbook PDFs; document kind is preserved. Returns source URL, labels, SHA256 and owner artifact path; no PDF bytes in model text. Inspect the original to verify TOC/sections. No arbitrary URL/path input or student completion inference.'),
    inputSchema:textbookPdfInput,annotations:{...readOnlyAnnotations,openWorldHint:true},
  },async input=>{
    if(!backend)return toolResult({status:'unavailable',code:'textbook_source_backend_not_configured',...limits});
    try{const observation=await backend.readTextbookPdf(input.schoolLevel,input.documentRef);
      if(observation.documentRef!==input.documentRef||observation.schoolLevel!==input.schoolLevel)throw Error('textbook_pdf_identity_rejected');
      const artifact=await preserveTextbookPdf(artifactRoot,observation);const {bytes:_bytes,...metadata}=observation;
      return toolResult({status:'observed',sourceMode:'live_source',...metadata,artifact,...limits});
    }catch(error){return failure(error);}
  });
}
