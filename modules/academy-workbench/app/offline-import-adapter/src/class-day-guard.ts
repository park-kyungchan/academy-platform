// Class-day guard for live academy reads.
//
// Governance rule encoded here: resolve a student's class days from evidence
// BEFORE attempting a live read, and skip non-class dates with an attributed
// reason instead of retrying. A bridge latch refusal
// (source_reconciliation_required_no_automatic_retry) is endpoint-level and
// must never be read as proof that a dated row is absent; a skipped
// non-class date is a caller decision, not source evidence.
// Pure offline functions: no source access, no guessing inside.
const WEEKDAY:Record<string,number>={'일':0,'월':1,'화':2,'수':3,'목':4,'금':5,'토':6};
function fail(condition:unknown,reason:string):asserts condition{if(!condition)throw Error(reason);}
export function parseClassDays(className:unknown):number[]{
 fail(typeof className==='string'&&className.length>0,'invalid_class_name');
 const days=new Set<number>();
 for(const char of className){if(char>='0'&&char<='9')continue;if(char==='반')continue;const day=WEEKDAY[char];fail(day!==undefined,'invalid_class_name');days.add(day);}
 fail(days.size>0,'invalid_class_name');
 return[...days].sort((a,b)=>a-b);
}
export interface LiveReadQuery{classDays:number[];lessonDate:string;basis:string;}
export interface LiveReadDecision{attempt:boolean;reason:string;}
export function shouldAttemptLiveRead(query:LiveReadQuery):LiveReadDecision{
 fail(query&&Array.isArray(query.classDays)&&query.classDays.length>0&&query.classDays.every(d=>Number.isInteger(d)&&d>=0&&d<=6),'invalid_class_days');
 fail(typeof query.lessonDate==='string'&&/^\d{4}-\d{2}-\d{2}$/.test(query.lessonDate),'invalid_lesson_date');
 const[y,m,day]=query.lessonDate.split('-').map(Number);
 const date=new Date(Date.UTC(y,m-1,day));
 fail(date.getUTCFullYear()===y&&date.getUTCMonth()===m-1&&date.getUTCDate()===day,'invalid_lesson_date');
 fail(typeof query.basis==='string'&&query.basis.length>0,'invalid_basis');
 if(query.classDays.includes(date.getUTCDay()))return{attempt:true,reason:`class_day_attempt: ${query.lessonDate} falls on a class day per ${query.basis}`};
 return{attempt:false,reason:`non_class_day_skip: ${query.lessonDate} is not a class day per ${query.basis}; do not attempt a live read`};
}
