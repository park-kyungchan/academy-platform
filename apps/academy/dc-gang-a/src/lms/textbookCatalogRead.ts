import {createHash} from 'node:crypto';

export const TEXTBOOK_CATALOG_PATH='/servlet/controller.coursemanage.CourseManageServlet?reqCmd=GaStudyAnswer';
export const MAX_TEXTBOOK_CATALOG_BYTES=2_000_000;
export const MAX_TEXTBOOK_SAMPLE_BYTES=32*1024*1024;
export const MAX_TEXTBOOK_REFERENCE_BYTES=64*1024*1024;
const sha=(bytes:string|Uint8Array)=>createHash('sha256').update(bytes).digest('hex');
export type TextbookSchoolLevel='elementary'|'middle'|'high';
export type TextbookDocumentKind='sample'|'answer'|'repair'|'daily'|'other';
export interface TextbookPdfEntry {
  kind:TextbookDocumentKind;documentRef:string;url:string;filename:string;sourceLabels:string[];
}
export interface TextbookCatalogObservation {
  schemaVersion:1;status:'observed';schoolLevel:TextbookSchoolLevel;observedAt:string;catalogSha256:string;entries:TextbookPdfEntry[];
  coverage:'all_observed_pdf_links_in_one_catalog_document';sourceRequests:1;
}
export interface TextbookPdfObservation {
  schoolLevel:TextbookSchoolLevel;kind:TextbookDocumentKind;documentRef:string;filename:string;sourceUrl:string;sourceLabels:string[];catalogSha256:string;
  observedAt:string;sha256:string;byteLength:number;bytes:Uint8Array;sourceRequests:2;
}
/** Only an actual fixed-catalog link can reach the public storage transport. */
export function qualifiedTextbookPdfUrl(literal:string):URL {
  let url:URL;try{url=new URL(literal);}catch{throw Error('textbook_pdf_link_rejected');}
  if(url.href!==literal||url.origin!=='https://storage.studyq.net'||url.username||url.password||url.search||url.hash
    ||!/^\/data\/answer\/(el|mi|hi)\/(book|workbook|fiveend)\/[A-Za-z0-9][A-Za-z0-9_.-]*\.pdf$/.test(url.pathname))
    throw Error('textbook_pdf_link_rejected');
  return url;
}
export function parseTextbookCatalog(html:string,parse:(html:string,mime:DOMParserSupportedType)=>Document,schoolLevel:TextbookSchoolLevel='middle'):TextbookCatalogObservation {
  if(/normal_session_error\.jsp|popup_session_error\.jsp|세션이 만료|<input\b[^>]*type\s*=\s*["']?password/i.test(html))
    throw Error('textbook_catalog_authentication_required');
  const doc=parse(html,'text/html'),entries=new Map<string,TextbookPdfEntry>();
  const levelLabel={elementary:'초등부',middle:'중등부',high:'고등부'}[schoolLevel];
  const selected=[...doc.querySelectorAll('.tab.tab-on')].filter(n=>['초등부','중등부','고등부'].includes((n.textContent??'').trim()));
  if(selected.length!==1||selected[0]!.textContent!.trim()!==levelLabel)throw Error('textbook_catalog_school_level_rejected');
  const labels=observedRowLabels(doc);
  for(const node of doc.querySelectorAll('[onclick],a[href]')){
    const candidates=[node.getAttribute('onclick')??'',node.getAttribute('href')??''];
    for(const code of candidates){
      const calls=[...code.matchAll(/fullUrlDownloadFile\s*\(\s*(['"])([^'"<>\s]+)\1\s*\)/g)];
      for(const match of calls){
        const literal=match[2]!;
        if(!literal.includes('/data/answer/'))continue;
        const url=qualifiedTextbookPdfUrl(literal),documentRef=sha(JSON.stringify([schoolLevel,url.href]));
        const row=node.closest('tr');
        const literalLabel=row?labels.get(row)??'':node.parentElement?.textContent??'';
        const label=literalLabel.replace(/\s+/g,' ').trim();
        if(!label||label.length>2000)throw Error('textbook_catalog_row_unqualified');
        const filename=url.pathname.split('/').at(-1)!;
        const kind:TextbookDocumentKind=/_sample_/.test(filename)?'sample':/_answer(?:_|\.)/.test(filename)?'answer':/_repair(?:_|\.)/.test(filename)?'repair':/_daily(?:_|\.)/.test(filename)?'daily':'other';
        const entry=entries.get(documentRef)??{kind,documentRef,url:url.href,filename,sourceLabels:[]};
        if(!entry.sourceLabels.includes(label))entry.sourceLabels.push(label);
        entries.set(documentRef,entry);
      }
    }
  }
  if(!entries.size||entries.size>1000)throw Error('textbook_catalog_pdf_links_unqualified');
  return {schemaVersion:1,status:'observed',schoolLevel,observedAt:new Date().toISOString(),catalogSha256:sha(html),
    entries:[...entries.values()],coverage:'all_observed_pdf_links_in_one_catalog_document',sourceRequests:1};
}
export async function boundedTextbookBody(response:Response,maximum:number,kind:'catalog'|'pdf'):Promise<Uint8Array>{
  const reject=async(code:string):Promise<never>=>{try{await response.body?.cancel();}catch{}throw Error(code);};
  if(response.status!==200||response.redirected)return reject('textbook_'+kind+'_response_rejected');
  const mime=(response.headers.get('content-type')??'').split(';')[0]!.trim().toLowerCase();
  if(!(kind==='catalog'?mime==='text/html':['application/pdf','application/octet-stream'].includes(mime)))
    return reject('textbook_'+kind+'_content_type_rejected');
  const header=response.headers.get('content-length'),size=header===null?null:Number(header);
  if(size!==null&&(!Number.isSafeInteger(size)||size<1||size>maximum))return reject('textbook_'+kind+'_size_rejected');
  const reader=response.body?.getReader();if(!reader)throw Error('textbook_'+kind+'_body_missing');
  const chunks:Uint8Array[]=[];let count=0;
  try{
    for(;;){const next=await reader.read();if(next.done)break;count+=next.value.byteLength;
      if(count>maximum){await reader.cancel();throw Error('textbook_'+kind+'_size_rejected');}chunks.push(next.value);}
  }finally{reader.releaseLock();}
  const encoded=response.headers.get('content-encoding');
  if(!count||(size!==null&&(!encoded||encoded==='identity')&&count!==size))throw Error('textbook_'+kind+'_size_rejected');
  const bytes=new Uint8Array(count);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.byteLength;}
  if(kind==='pdf'&&(Buffer.from(bytes.subarray(0,5)).toString('ascii')!=='%PDF-'
    ||!Buffer.from(bytes.subarray(Math.max(0,count-2048))).includes(Buffer.from('%%EOF'))))throw Error('textbook_pdf_signature_rejected');
  return bytes;
}

export function textbookPdfDigest(bytes:Uint8Array){return sha(bytes);}

/** Carry only actual rowspans/colspans; no inferred textbook hierarchy. */
function observedRowLabels(doc:Document):Map<Element,string>{
 const result=new Map<Element,string>();
 for(const table of doc.querySelectorAll('table')){
  const spans=new Map<number,{text:string;remaining:number}>();
  for(const row of table.rows){
   if(row.closest('table')!==table)continue;
   const cells=new Map<number,string>();
   for(const [column,value] of spans){cells.set(column,value.text);value.remaining--;if(value.remaining===0)spans.delete(column);}
   let column=0;
   for(const cell of row.cells){
    while(cells.has(column))column++;
    const width=Math.min(cell.colSpan,100),height=Math.min(cell.rowSpan,1000),text=(cell.textContent??'').replace(/\s+/g,' ').trim();
    for(let i=0;i<width;i++){cells.set(column+i,text);if(height>1)spans.set(column+i,{text,remaining:height-1});}
    column+=width;
   }
   result.set(row,[...cells].sort((a,b)=>a[0]-b[0]).map(v=>v[1]).join(' '));
  }
 }
 return result;
}
