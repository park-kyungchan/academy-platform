/** Builds data/textbook-toc.json (academy.textbook-toc/1) from a text-only extraction of the
 * catalog sample PDFs: one `<filename>.json` per PDF with schoolLevel, filename, sourceSha256,
 * pdfTitle, outline [{page,text}] and tocPages [{pdfPage,text}] (pdftotext -layout of the first
 * pages that matched a TOC word). Deterministic: records sorted by filename, no clock, no host path.
 * Usage: bun scripts/build-textbook-toc.ts --raw <dir> [--out <file>] */
import {readdirSync,readFileSync,writeFileSync,renameSync} from 'node:fs';
import {resolve} from 'node:path';

type Raw={schoolLevel:'elementary'|'middle'|'high';filename:string;sourceSha256:string;pdfTitle:string|null;
  outline:{page:number|null;text:string}[];tocPages:{pdfPage:number;text:string}[]};
export type TocUnit={chapter:string;chapterTitle:string|null;code:string|null;title:string;pages:string};
export type TocRecord={book:string;aliases:string[];pdfTitle:string|null;schoolLevel:Raw['schoolLevel'];filename:string;
  sourceSha256:string;noTextToc:boolean;toc:string|null;units:TocUnit[]|null;outline:string[]};

// The file name's series word is not the brand (`develop` is 가우스); `aliases` keeps the other names.
const elementary:Record<string,{brand:string;aliases:string[]}>={basic:{brand:'가우스',aliases:['발전']},develop:{brand:'가우스',aliases:['발전']},
  ability:{brand:'다빈치',aliases:[]},highest:{brand:'오일러',aliases:['최상위']},sigma:{brand:'개념연산',aliases:['시그마']}};
const brands:Record<string,string>={gauss:'가우스',davinci:'다빈치',euler:'오일러',pascal:'파스칼',sigma:'시그마'};
// High school books keep the catalog's subject name; `stats` is the 2022 revision beside the older `statistics`.
const subjects:Record<string,{name:string;aliases:string[]}>={'10_high':{name:'수학(상)',aliases:[]},'10_low':{name:'수학(하)',aliases:[]},
  '10_one':{name:'공통수학1',aliases:[]},'10_two':{name:'공통수학2',aliases:[]},'11_one':{name:'수학Ⅰ',aliases:['수학1']},'11_two':{name:'수학Ⅱ',aliases:['수학2']},
  '11_statistics':{name:'확률과 통계',aliases:[]},'11_stats':{name:'확률과 통계(22개정)',aliases:['확률과 통계','확률과통계']},
  '11_calculus':{name:'미적분',aliases:[]},'11_calculus1':{name:'미적분1',aliases:['미적분Ⅰ']},'11_calculus2':{name:'미적분2',aliases:['미적분Ⅱ']},
  '11_algebra':{name:'대수',aliases:[]},'11_geometry':{name:'기하',aliases:[]}};
const markers=/^(책갈피|EXAM_AREA|OBJ_AREA)$|^정답|_시작$|_끝$|-(end|start)$/;

function naming(raw:Raw,title:string|null,seriesHasVolumes:Set<string>):{book:string;aliases:string[];series:string}{
  const low=/^g([2-9])_([a-z]+)_sample_([12])_([1-4])\.pdf$/.exec(raw.filename);
  if(low){
    const [,grade,series,term,file]=low,middle=raw.schoolLevel==='middle';
    const name=middle?{brand:brands[series!],aliases:[] as string[]}:elementary[series!];
    if(!name)throw Error('toc_series_unknown:'+raw.filename);
    const gradeTerm=(middle?'중'+(Number(grade)-6):grade)+'-'+term;
    if(title&&!title.replace(/\s/g,'').includes(gradeTerm))throw Error('toc_title_grade_mismatch:'+raw.filename);
    // The volume is written when the title states it; without a usable title, when the series' titled books do.
    const volume=title?/(\d)권/.exec(title)?.[1]:seriesHasVolumes.has(raw.schoolLevel+series)?file:undefined;
    return {book:name.brand+' '+gradeTerm+(volume?'-'+volume:''),aliases:name.aliases,series:raw.schoolLevel+series};
  }
  const high=/^g(1[01])_([a-z0-9]+)_([a-z]+)_sample_(\d)\.pdf$/.exec(raw.filename);
  const subject=high&&subjects[high[1]+'_'+high[2]],brand=high&&brands[high[3]!];
  if(!high||!subject||!brand)throw Error('toc_series_unknown:'+raw.filename);
  return {book:brand+' '+subject.name+(high[3]==='sigma'?'':' '+high[4]+'권'),aliases:subject.aliases,series:'high'+high[3]};
}

// A TOC page: the plan table (`쪽수` and `학습 계획`), or, within the first 12 pages, a page of at least
// three `title  page` lines (dotted leaders allowed) with no exercise wording.
const pageLine=/\S(?:\s{2,}|\s*-{3,}\s*)\d{1,3}(?:\s*~\s*\d{1,3})?$/;
export const tocPage=(page:{pdfPage:number;text:string})=>/쪽수/.test(page.text)&&/학습\s*계획/.test(page.text)
  ||page.pdfPage<=12&&page.text.split('\n').filter(l=>pageLine.test(l.trimEnd())).length>=3
    &&!/써넣|구하시오|구하여라|고르시오|구하면|①|하시오|하여라|빈칸|\?/.test(page.text);
function cleanToc(pages:string[]):string{
  return pages.map(text=>text.split('\n').map(line=>line.replace(/(\s+월\s+일)+\s*$/,'').replace(/\s+$/,'')).join('\n')).join('\n\n')
    .replace(/\n{3,}/g,'\n\n').trim();
}

// Units from the TOC text; null wherever the layout is not understood. Two layouts:
// - plan tables: a minor line is `N. title  page`, N continuing the previous minor or restarting at 1;
//   text left of it, and lines without a page, form the chapter column (`4. 약분과` / `통분`); a Roman
//   heading (`Ⅰ. 수와 연산`) names the 대단원. Two minors may share one page printed between them.
// - coded lists: `1-1 title ---- 10`, one or two columns; the code is printed.
const romanValues:Record<string,number>={'Ⅰ':1,'Ⅱ':2,'Ⅲ':3,'Ⅳ':4,'Ⅴ':5,'Ⅵ':6,'Ⅶ':7,'Ⅷ':8,'Ⅸ':9,'Ⅹ':10};
const page='\\d{1,3}(?:\\s*~\\s*\\d{1,3})?',assessmentWords=/대단원|총괄|Master|만점도전/;
const clean=(s:string)=>s.replace(/\s+/g,' ').trim(),pages=(s:string)=>s.replace(/\s/g,''),firstPage=(u:TocUnit)=>Number(u.pages.split('~')[0]);
const headerLine=/쪽수|학습\s*계획|확인란|지키도록|Contents/;
type Minor={line:number;n:number;title:string;pages:string|null;group:number};
type Label={line:number;col:number;text:string};

export function parseUnits(toc:string):TocUnit[]|null{
  const lines=toc.split('\n').map(line=>line.replace(/\s*-{3,}\s*/g,'   ').replace(/\s+$/,''));
  const units=lines.some(l=>/(?:^|\s)[1-9]\d?-\d{1,2}\.?\s+\S/.test(l))?codedUnits(lines):planUnits(lines);
  if(!units?.length)return null;
  const codes=units.flatMap(u=>u.code?[u.code]:[]);
  return new Set(codes).size===codes.length?units:null;
}

function codedUnits(lines:string[]):TocUnit[]|null{
  const units:TocUnit[]=[],item=new RegExp('(?:^|\\s)([1-9]\\d?)-(\\d{1,2})\\.?\\s+(\\S.*?)\\s{2,}('+page+')(?=\\s|$)','g');
  for(const line of lines){
    if(headerLine.test(line))continue;
    const found=[...line.matchAll(item)];
    if([...line.matchAll(/(?:^|\s)[1-9]\d?-\d{1,2}\.?\s+\S/g)].length!==found.length)return null; // a wrapped title
    for(const m of found)units.push({chapter:m[1]!,chapterTitle:null,code:m[1]+'-'+Number(m[2]),title:clean(m[3]!),pages:pages(m[4]!)});
    let rest=line;for(const m of found)rest=rest.replace(m[0],' '.repeat(m[0].length));
    const a=new RegExp('^\\s*(\\S.*?)\\s{2,}('+page+')$').exec(rest);
    if(a&&assessmentWords.test(a[1]!))units.push({chapter:'',chapterTitle:null,code:null,title:clean(a[1]!),pages:pages(a[2]!)});
  }
  // Two columns read across: order by code. Every chapter numbers its units 1..k, and pages rise in code order.
  const coded=units.filter(u=>u.code).sort((x,y)=>Number(x.chapter)-Number(y.chapter)||Number(x.code!.split('-')[1])-Number(y.code!.split('-')[1]));
  const ok=coded.length>0&&coded.every((u,i)=>{const n=Number(u.code!.split('-')[1]),before=coded[i-1];
    return (before?.chapter===u.chapter?n===Number(before.code!.split('-')[1])+1:n===1)&&(!before||firstPage(u)>=firstPage(before));});
  if(!ok)return null;
  // An assessment follows the last unit that starts before its page.
  const ordered:TocUnit[]=[...coded];
  for(const a of units.filter(u=>!u.code)){
    const at=ordered.findLastIndex(u=>firstPage(u)<=firstPage(a));if(at<0)return null;
    ordered.splice(at+1,0,{...a,chapter:ordered[at]!.chapter});
  }
  return ordered;
}

function planUnits(lines:string[]):TocUnit[]|null{
  // The column of the minor units: where most `N. title  page` lines start.
  const columns=new Map<number,number>();
  for(const line of lines){const m=[...line.matchAll(new RegExp('(?:^|\\s)(?=\\d{1,2}\\.\\s+\\S.*\\s{2,}'+page+'$)','g'))].at(-1);
    if(m){const col=m.index!+(m[0].length);columns.set(col,(columns.get(col)??0)+1);}}
  const minorCol=[...columns].sort((a,b)=>b[1]-a[1]||a[0]-b[0])[0]?.[0];
  if(minorCol===undefined)return null;
  const minors:Minor[]=[],labels:Label[]=[],romans:Label[]=[],lone:{line:number;pages:string;used:boolean}[]=[],assessments:{line:number;title:string;pages:string}[]=[];
  let previous=0,group=-1;
  for(const [index,line] of lines.entries()){
    if(!line.trim()||headerLine.test(line))continue;
    const section=new RegExp('^\\s*§\\s*(.+?)(?:\\s{2,}('+page+'))?$').exec(line);
    if(section){if(assessmentWords.test(section[1]!)){if(!section[2])return null;assessments.push({line:index,title:clean(section[1]!),pages:pages(section[2])});}continue;}
    const alone=new RegExp('^\\s+('+page+')$').exec(line);
    if(alone){lone.push({line:index,pages:pages(alone[1]!),used:false});continue;}
    let minor:Minor|null=null,left=line;
    for(const start of [...line.matchAll(/(?:^|\s)(\d{1,2})\.\s/g)].reverse()){
      const n=Number(start[1]),at=start.index!+(/^\s/.test(start[0])?1:0);if(n!==previous+1&&n!==1)continue;
      const m=new RegExp('^\\d{1,2}\\.\\s+(.+?)\\s{2,}('+page+')$').exec(line.slice(at))??(Math.abs(at-minorCol)<=2?/^\d{1,2}\.\s+(.+)$/.exec(line.slice(at)):null);
      if(!m)continue;
      if(n===1&&previous!==0||group<0)group++;
      minor={line:index,n,title:clean(m[1]!),pages:m[2]?pages(m[2]):null,group};left=line.slice(0,at);previous=n;break;
    }
    if(minor)minors.push(minor);
    else{
      const a=new RegExp('^\\s*(\\S.*?)\\s{2,}('+page+')$').exec(line);
      if(a&&assessmentWords.test(a[1]!)){assessments.push({line:index,title:clean(a[1]!),pages:pages(a[2]!)});continue;}
      if(a)return null; // a page line that is neither a minor unit nor an assessment
    }
    const text=left.trim();if(!text)continue;
    (/^[ⅠⅡⅢⅣⅤⅥⅦⅧⅨⅩ]+\.\s*\S/.test(text)?romans:labels).push({line:index,col:left.length-left.trimStart().length,text});
  }
  // A minor without a page shares the page printed alone on the line next to it.
  for(const m of minors.filter(m=>m.pages===null)){
    const near=lone.filter(p=>Math.abs(p.line-m.line)===1).sort((a,b)=>a.line-b.line)[0];if(!near)return null;
    m.pages=near.pages;near.used=true;
  }
  if(!minors.length||lone.some(p=>!p.used))return null;
  const groups=[...new Set(minors.map(m=>m.group))].map(g=>{const own=minors.filter(m=>m.group===g);
    return {own,first:own[0]!.line,last:own.at(-1)!.line,labels:[] as Label[],romans:[] as Label[]};});
  const distance=(line:number,g:{first:number;last:number})=>line<g.first?g.first-line:line>g.last?line-g.last:0;
  const nearest=(line:number)=>groups.reduce((best,g,i)=>distance(line,g)<distance(line,groups[best]!)?i:best,0);
  for(const label of labels)groups[nearest(label.line)]!.labels.push(label);
  // A heading at or right of the minor column stands above its group; one in the chapter column sits beside it.
  for(const roman of romans){
    const i=roman.col>=minorCol-2?groups.findIndex(g=>g.first>roman.line):nearest(roman.line);if(i<0)return null;
    groups[i]!.romans.push(roman);
  }
  const romanOf=(label:Label)=>{const m=/^([ⅠⅡⅢⅣⅤⅥⅦⅧⅨⅩ]+)\.\s*(.+)$/.exec(label.text)!;return {chapter:String(romanValues[m[1]!]),chapterTitle:clean(m[2]!)};};
  const units:TocUnit[]=[];let inherited:Label|null=null;
  const chapterOf=groups.map(g=>{
    if(g.romans.length>1)return null;
    inherited=g.romans[0]??inherited;return inherited;
  });
  for(const [i,g] of groups.entries()){
    let chapters:{chapter:string;chapterTitle:string;code:(n:number)=>string}[]|null=null;
    if(g.romans.length>1){
      // Minors numbered across several 대단원 (중단원 lists): each takes the nearest heading; the code is its number.
      chapters=g.own.map(m=>{const r=g.romans.reduce((a,b)=>Math.abs(b.line-m.line)<Math.abs(a.line-m.line)?b:a);return {...romanOf(r),code:(n:number)=>String(n)};});
    }else{
      const roman=chapterOf[i],shared=roman&&chapterOf.filter(r=>r===roman).length>1;
      let c:{chapter:string;chapterTitle:string}|null=null;
      if(roman&&!shared)c=romanOf(roman);
      else{const text=clean(g.labels.map(l=>l.text).join(' ')),m=/^(\d{1,2})\.\s*(.+)$/.exec(text);
        if(m&&g.labels.filter(l=>/^\d{1,2}\.\s/.test(l.text)).length===1)c={chapter:String(Number(m[1])),chapterTitle:m[2]!};}
      if(!c)return null;
      chapters=g.own.map(()=>({...c!,code:(n:number)=>c!.chapter+'-'+n}));
    }
    // Assessments after the group's first minor and before the next group follow the minor above them.
    const end=groups[i+1]?.first??Infinity,entries:{line:number;unit:TocUnit}[]=g.own.map((m,k)=>{const c=chapters![k]!;
      return {line:m.line,unit:{chapter:c.chapter,chapterTitle:c.chapterTitle,code:c.code(m.n),title:m.title,pages:m.pages!}};});
    for(const a of assessments.filter(a=>a.line>g.first&&a.line<end)){const above=entries.filter(e=>e.unit.code&&e.line<a.line).at(-1)!.unit;
      entries.push({line:a.line,unit:{chapter:above.chapter,chapterTitle:above.chapterTitle,code:null,title:a.title,pages:a.pages}});}
    units.push(...entries.sort((x,y)=>x.line-y.line).map(e=>e.unit));
  }
  if(assessments.some(a=>a.line<groups[0]!.first)||units.some((u,i)=>i>0&&firstPage(u)<firstPage(units[i-1]!)))return null;
  return units;
}

function outlineUnits(raw:Raw):string[]{
  const items=raw.outline.slice(1).map(o=>o.text.trim().replace(/-대단원$/,''))
    .filter(t=>t&&!markers.test(t)&&/^(?:\d{1,2}-\d{1,2}\.?\s+\S|\d{1,2}\.\s*\S|[ⅠⅡⅢⅣⅤⅥⅦⅧⅨⅩ]+\.\s*\S)/.test(t));
  return [...new Set(items)];
}

export function buildTextbookToc(rawDir:string):string{
  const raws=readdirSync(rawDir).filter(f=>f.endsWith('.pdf.json')).sort().map(f=>JSON.parse(readFileSync(resolve(rawDir,f),'utf8')) as Raw);
  const titleOf=(raw:Raw)=>{const t=raw.outline[0]?.text.trim()??raw.pdfTitle;return t&&/[가-힣]/.test(t)?t:null;};
  const seriesHasVolumes=new Set(raws.flatMap(raw=>{const m=/^g\d_([a-z]+)_/.exec(raw.filename),t=titleOf(raw);return m&&t&&/\d권/.test(t)?[raw.schoolLevel+m[1]]:[];}));
  const records:TocRecord[]=raws.map(raw=>{
    const title=titleOf(raw),{book,aliases}=naming(raw,title,seriesHasVolumes);
    const pages=raw.tocPages.filter(tocPage).sort((a,b)=>a.pdfPage-b.pdfPage).map(p=>p.text);
    const toc=pages.length?cleanToc(pages):null,outline=toc?[]:outlineUnits(raw);
    return {book,aliases,pdfTitle:raw.outline[0]?.text.trim()??raw.pdfTitle,schoolLevel:raw.schoolLevel,filename:raw.filename,sourceSha256:raw.sourceSha256,
      noTextToc:!toc&&!outline.length,toc,units:toc?parseUnits(toc):null,outline};
  }).sort((a,b)=>a.filename<b.filename?-1:a.filename>b.filename?1:0);
  return JSON.stringify({schemaVersion:'academy.textbook-toc/1',source:'catalog sample PDFs, text layer only (pdftotext -layout, PDF outline); no image read',records},null,1)+'\n';
}

if(import.meta.main){
  const arg=(name:string)=>{const i=process.argv.indexOf(name);return i>0?process.argv[i+1]:undefined;};
  const rawDir=arg('--raw');if(!rawDir)throw Error('usage: bun scripts/build-textbook-toc.ts --raw <dir> [--out <file>]');
  const out=resolve(arg('--out')??resolve(import.meta.dir,'../data/textbook-toc.json'));
  writeFileSync(out+'.tmp',buildTextbookToc(rawDir));renameSync(out+'.tmp',out);
}
