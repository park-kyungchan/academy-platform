import {z} from 'zod';
import type {McpServer} from '@modelcontextprotocol/server';
import {toolResult} from './tool-result.ts';
import {readOnlyAnnotations} from './contracts.ts';
import {academyRoutingDescription} from './routing-context.ts';
// Inlined by `bun build`: the installed endpoint reads no data path at run time.
import tocData from '../data/textbook-toc.json' with {type:'json'};

/** academy.textbook-toc/1, built by scripts/build-textbook-toc.ts from the text layer of the catalog sample PDFs. */
type TocUnit={chapter:string;chapterTitle:string|null;code:string|null;title:string;pages:string};
type TocRecord={book:string;aliases:string[];pdfTitle:string|null;schoolLevel:'elementary'|'middle'|'high';filename:string;
  sourceSha256:string;noTextToc:boolean;toc:string|null;units:TocUnit[]|null;outline:string[]};
const records=(tocData as {records:TocRecord[]}).records;

export const textbookTocInput=z.object({
  book:z.string().trim().min(1).max(200),
  unit:z.string().trim().regex(/^\d{1,2}(?:-\d{1,2})?$/).optional(),
}).strict();
export const textbookTocDescription='Units, pages and chapter assessments (대단원 총괄평가) of a catalog textbook, from pinned TOC data kept as text: no LMS request, no network, no PDF download, no image read. Use it first whenever a book\'s units or pages are needed. book: a canonical name (가우스 5-1-2, 가우스 중1-1-2, 가우스 공통수학1 1권), an LMS bound-book label as listed for the student, a name without volume (가우스 5-1 gives every volume) or a sample file name. unit: "6" or "6-5" returns only chapter 6. Answers observed with at most 4 books (toc text, units with code/title/pages, or outline items), or not_found with canonical names to choose from. noTextToc: no TOC text is kept for that book: ask the teacher; do not download the PDF.';

const compact=(s:string)=>s.normalize('NFC').replace(/\s+/g,'');
type Parsed={brand:string;level:string;grade:string;term:string;volume:string|null}|{brand:string;subject:string;volume:string|null};
function parseBook(book:string):Parsed{
  const low=/^(\S+) (중)?(\d)-(\d)(?:-(\d))?$/.exec(book);
  if(low)return {brand:low[1]!,level:low[2]??'',grade:low[3]!,term:low[4]!,volume:low[5]??null};
  const high=/^(\S+) (.+?)(?: (\d)권)?$/.exec(book)!;
  return {brand:high[1]!,subject:high[2]!,volume:high[3]??null};
}
const parsed=new Map(records.map(r=>[r,parseBook(r.book)]));
// Words that name a series: brands and the aliases that are not subject names.
const seriesWords=(r:TocRecord)=>[parsed.get(r)!.brand,...r.aliases.filter(a=>!/수학|미적분|확률|통계|대수|기하/.test(a))];
const allSeriesWords=[...new Set(records.flatMap(seriesWords))];

function match(query:string):{found:TocRecord[];suggestions:string[]}{
  const q=query.normalize('NFC').replace(/\s+/g,' ').trim(),c=compact(q);
  const exact=records.filter(r=>r.filename===q||r.filename===q+'.pdf'||compact(r.book)===c);
  if(exact.length)return {found:exact,suggestions:[]};
  const named=allSeriesWords.filter(w=>c.includes(compact(w)));
  const gradeTerm=/(초|중|고)?\s*(\d)\s*-\s*(\d)(?:\s*-\s*(\d))?/.exec(q);
  const volume=gradeTerm?.[4]??/(\d)\s*권/.exec(q)?.[1]??null;
  const brandOk=(r:TocRecord)=>!named.length||seriesWords(r).some(w=>named.includes(w));
  // 가우스플러스 is a separate workbook with no sample PDF: never answer it with a 가우스 book.
  let found=/플러스/.test(c)?[]:records.filter(r=>{
    const p=parsed.get(r)!;if(!brandOk(r))return false;
    if('grade' in p)return !!gradeTerm&&p.grade===gradeTerm[2]&&p.term===gradeTerm[3]&&(gradeTerm[1]==='중')===(p.level==='중')
      &&gradeTerm[1]!=='고'&&(!volume||!p.volume||p.volume===volume);
    return [p.subject,...r.aliases].some(s=>c.includes(compact(s)))&&(!volume||p.volume===volume);
  });
  // The longest subject named wins: 미적분1 over 미적분.
  const subjectLength=(r:TocRecord)=>{const p=parsed.get(r)!;return 'subject' in p?Math.max(...[p.subject,...r.aliases].filter(s=>c.includes(compact(s))).map(s=>compact(s).length)):0;};
  const longest=Math.max(0,...found.map(subjectLength));found=found.filter(r=>subjectLength(r)===longest);
  if(found.length)return {found,suggestions:[]};
  // Suggestions: the same brand and grade first, then the same grade, then the same brand.
  const sameGrade=(r:TocRecord)=>{const p=parsed.get(r)!;return !!gradeTerm&&'grade' in p&&p.grade===gradeTerm[2]&&(gradeTerm[1]==='중')===(p.level==='중');};
  const score=(r:TocRecord)=>(named.length&&brandOk(r)?1:0)+(sameGrade(r)?2:0);
  const suggestions=records.filter(r=>score(r)>0).sort((a,b)=>score(b)-score(a)||(a.book<b.book?-1:a.book>b.book?1:0)).map(r=>r.book).slice(0,20);
  return {found:[],suggestions};
}

/** Only the lines of one chapter: its units, the TOC lines that print them, its outline items. */
function forUnit(record:TocRecord,unit:string){
  const chapter=unit.split('-')[0]!;
  const outline=record.outline.filter(item=>new RegExp('^(?:'+chapter+'-\\d|0?'+chapter+'\\.)').test(item));
  if(!record.units)return {...record,outline,unitFilter:{unit,chapter,applied:'outline_only'}};
  const units=record.units.filter(u=>u.chapter===chapter);
  const toc=record.toc&&record.toc.split('\n').filter(line=>units.some(u=>line.includes(u.title)&&line.includes(u.pages.split('~')[0]!))).join('\n');
  return {...record,toc:toc||null,units,outline,unitFilter:{unit,chapter,applied:'units'}};
}

const effects={sourceMode:'pinned_toc_data',sourceRequests:0,pdfDownloads:0,imageReads:0,academyWrites:0};
export function registerTextbookTocTool(server:McpServer){
  server.registerTool('academy_textbook_toc_get',{
    title:'Read a textbook TOC (units, pages, chapter assessments) from pinned data',
    description:academyRoutingDescription('academy_textbook_toc_get',textbookTocDescription),
    inputSchema:textbookTocInput,annotations:readOnlyAnnotations,
  },async input=>{
    const {found,suggestions}=match(input.book);
    if(!found.length)return toolResult({status:'not_found',book:input.book,suggestions,...effects,
      next:'Choose one of the suggestions, or ask the teacher which book; do not download a PDF for its TOC.'});
    const selected=found.slice(0,4).map(r=>input.unit?forUnit(r,input.unit):r);
    return toolResult({status:'observed',...effects,matchCount:found.length,records:selected,
      ...(found.length>4?{otherBooks:found.slice(4).map(r=>r.book)}:{}),
      ...(selected.some(r=>r.noTextToc)?{next:'A book marked noTextToc has no TOC text here: ask the teacher for its units and pages.'}:{})});
  });
}
