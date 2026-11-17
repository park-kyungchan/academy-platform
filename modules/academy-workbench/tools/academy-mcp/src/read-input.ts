import {randomUUID} from 'node:crypto';
import {z} from 'zod';

/** Input words of the read tools, as a teacher or a model writes them.
 * Request identity is optional, dates take relative and month-day forms, and
 * a book name may be loose. Each is resolved here before any source read. */

/** Optional request identity: the server fills what the caller leaves out. */
const identityText=z.string().min(1).max(256).refine(value=>value.trim().length>0&&!/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(value),'nonempty_safe_text_required');
export const requestIdentityShape={
  purpose:identityText.optional().describe('Optional. When omitted the server uses the tool name.'),
  currentRequestRef:identityText.optional().describe('Optional. When omitted the server makes a new reference for this call and returns it.'),
};
export type WithRequest<T>=Omit<T,'purpose'|'currentRequestRef'>&{purpose:string;currentRequestRef:string};
export function withRequestIdentity<T extends {purpose?:string|undefined;currentRequestRef?:string|undefined}>(tool:string,input:T):WithRequest<T>{
  return {...input,purpose:input.purpose??tool,currentRequestRef:input.currentRequestRef??'auto-'+randomUUID()};
}

/** The one refusal of a date the server cannot read; the code names the accepted forms. */
export const dateRefusalCode='date_form_unsupported_use_yyyy_mm_dd_today_yesterday_or_month_day';
export const academyDateInputSchema=z.string().trim().min(1).max(40).describe('YYYY-MM-DD, today/오늘, yesterday/어제, M월 D일 (a weekday such as (목) may follow and is not checked), M/D or M-D. Resolved in Asia/Seoul: a month-day takes the current year, or the previous year when that date would be more than 31 days after today. The answer states the resolved YYYY-MM-DD.');

export function academyBusinessDate(now:number):string {
  return new Intl.DateTimeFormat('en-CA',{timeZone:'Asia/Seoul',year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date(now));
}
const calendarDate=(year:number,month:number,day:number)=>{
  const value=`${String(year).padStart(4,'0')}-${String(month).padStart(2,'0')}-${String(day).padStart(2,'0')}`;
  const parsed=new Date(value+'T00:00:00Z');
  return Number.isFinite(parsed.valueOf())&&parsed.toISOString().slice(0,10)===value?value:null;
};
const addDays=(date:string,days:number)=>new Date(Date.parse(date+'T00:00:00Z')+days*86400000).toISOString().slice(0,10);

/** One date member to `YYYY-MM-DD` in Asia/Seoul, or the one refusal code. */
export function resolveAcademyDate(value:string,now:number):string{
  const text=value.normalize('NFKC').trim(),today=academyBusinessDate(now);
  const iso=/^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
  if(iso){const date=calendarDate(Number(iso[1]),Number(iso[2]),Number(iso[3]));if(date)return date;throw Error(dateRefusalCode);}
  const word=text.toLowerCase();
  if(word==='today'||word==='오늘')return today;
  if(word==='yesterday'||word==='어제')return addDays(today,-1);
  const monthDay=/^(\d{1,2})\s*월\s*(\d{1,2})\s*일\s*(?:\(\s*[월화수목금토일](?:요일)?\s*\)|[월화수목금토일]요일)?$/u.exec(text)??/^(\d{1,2})\s*[/-]\s*(\d{1,2})$/.exec(text);
  if(!monthDay)throw Error(dateRefusalCode);
  const month=Number(monthDay[1]),day=Number(monthDay[2]),year=Number(today.slice(0,4));
  const current=calendarDate(year,month,day);
  if(!current)throw Error(dateRefusalCode);
  if(Date.parse(current+'T00:00:00Z')-Date.parse(today+'T00:00:00Z')<=31*86400000)return current;
  const previous=calendarDate(year-1,month,day);
  if(!previous)throw Error(dateRefusalCode);
  return previous;
}

/** Loose book names. The brand (가우스, 다빈치 …), grade and term (5-1,
 * 초5-1, 5학년 1학기, 중1-1) and volume (-2, 2권) are read from a name;
 * every other word of the request must appear in the bound label. */
type Grade={level:string|null;grade:string;term:string|null};
type BookFeatures={grades:Grade[];volumes:string[];words:string[]};
const levelOf=(value:string|undefined)=>!value?null:value.startsWith('초')?'초':value.startsWith('중')?'중':'고';
const stopWords=new Set(['교재','책','수학','학년','학기','권','초등','중등','고등','중학','개정','과정']);
function bookFeatures(text:string):BookFeatures{
  let rest=text.normalize('NFKC').replace(/[‐‑‒–—―−~]/g,'-').replace(/\(?\s*(?:20)?\d{2}\s*개정\s*\)?/g,' ');
  const grades:Grade[]=[],volumes:string[]=[];
  const level='(초등?|중등?|중학교?|고등?)?\\s*';
  rest=rest.replace(new RegExp(`${level}(\\d)\\s*학년(?:\\s*(\\d)\\s*학기)?`,'gu'),(_m,l,g,t)=>{grades.push({level:levelOf(l),grade:g,term:t??null});return ' ';});
  rest=rest.replace(new RegExp(`(?<![\\d-])${level}(\\d)\\s*-\\s*(\\d)(?:\\s*-\\s*(\\d{1,2}))?(?![\\d-])`,'gu'),(_m,l,g,t,v)=>{grades.push({level:levelOf(l),grade:g,term:t});if(v)volumes.push(String(Number(v)));return ' ';});
  rest=rest.replace(/(\d{1,2})\s*권/gu,(_m,v)=>{volumes.push(String(Number(v)));return ' ';});
  const words=rest.split(/[^\p{L}]+/u).filter(word=>word.length>=2&&!stopWords.has(word));
  return {grades,volumes,words};
}
function agrees(request:BookFeatures,label:string):boolean{
  const bound=bookFeatures(label),compact=label.normalize('NFKC').replace(/\s+/g,'');
  return request.grades.every(g=>bound.grades.some(b=>b.grade===g.grade&&(!g.term||!b.term||b.term===g.term)&&(!g.level||!b.level||b.level===g.level)))
    &&request.volumes.every(v=>bound.volumes.includes(v))
    &&request.words.every(word=>compact.includes(word));
}
export type BookChoice<B>={kind:'selected';binding:B;match:'exact'|'loose'}|{kind:'interview';candidates:B[];match:'several'|'none'};
/** The exact label wins; otherwise the bindings whose label agrees with the
 * loose name are kept. One is used; several or none ask the teacher. */
export function chooseBookBinding<B extends {bookLabel:string}>(requested:string,bindings:readonly B[]):BookChoice<B>{
  const exact=bindings.filter(b=>b.bookLabel===requested);
  if(exact.length===1)return {kind:'selected',binding:exact[0]!,match:'exact'};
  if(exact.length>1)return {kind:'interview',candidates:exact,match:'several'};
  const features=bookFeatures(requested);
  const informative=features.grades.length+features.volumes.length+features.words.length>0;
  const agreeing=informative?bindings.filter(b=>agrees(features,b.bookLabel)):[];
  if(agreeing.length===1)return {kind:'selected',binding:agreeing[0]!,match:'loose'};
  return agreeing.length?{kind:'interview',candidates:agreeing,match:'several'}:{kind:'interview',candidates:[...bindings],match:'none'};
}
export type BookSelection={requested:string;boundLabel:string;match:'exact'|'loose'}|{requested:string;match:'several'|'none'};
export const bookSelectionOf=<B extends {bookLabel:string}>(requested:string,choice:BookChoice<B>):BookSelection=>
  choice.kind==='selected'?{requested,boundLabel:choice.binding.bookLabel,match:choice.match}:{requested,match:choice.match};
