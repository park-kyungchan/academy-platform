import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import tls from 'node:tls';
import { syncBuiltinESMExports } from 'node:module';

function deny(): never {
  process.stderr.write('verification_outbound_network_attempt_blocked\n');
  throw Error('verification_outbound_network_forbidden');
}
globalThis.fetch = Object.assign(deny, { preconnect: deny });
http.request = deny as typeof http.request;
http.get = deny as typeof http.get;
https.request = deny as typeof https.request;
https.get = deny as typeof https.get;
// A connection to a Unix socket path stays local and is allowed; a port or a host is denied.
function isUnixPath(args: unknown[]): boolean {
  let first = args[0];
  if (Array.isArray(first)) first = first[0];
  if (typeof first === 'string') return Number.isNaN(Number(first)) || first.trim() === '';
  if (typeof first === 'object' && first !== null) {
    const options = first as {path?: unknown; port?: unknown};
    return typeof options.path === 'string' && options.port === undefined;
  }
  return false;
}
function guardUnixOnly<F extends (...args: any[]) => any>(original: F): F {
  return function guarded(this: unknown, ...args: unknown[]) {
    if (!isUnixPath(args)) return deny();
    return original.apply(this, args);
  } as unknown as F;
}
net.connect = guardUnixOnly(net.connect);
net.createConnection = guardUnixOnly(net.createConnection);
net.Socket.prototype.connect = guardUnixOnly(net.Socket.prototype.connect);
tls.connect = deny as typeof tls.connect;
syncBuiltinESMExports();

/** I1 `write_forbidden`: every LMS request is classed by its named route and operation, as
 * apps/academy/dc-gang-a/src/lms/lmsRouteRegistry.ts and endpointCandidates.ts class them
 * (specs/academy-mcp/tool-surface.md "Request classes"), never by its HTTP method: the
 * DailyRecord field writes are GETs and the reads below include POST search forms. The
 * operation is the `p_process` or `reqCmd` of the query, else of a form body. A route or an
 * operation that this table does not class is write. */
export type LmsRequestRole='login'|'read'|'write';
const servlet='/servlet/controller.';
const routeRoles:Readonly<Record<string,Readonly<Record<string,LmsRequestRole>>>>={
 '/c_login.jsp':{'':'login'},
 [servlet+'login.LoginServlet']:{c_login:'login'},
 [servlet+'coursemanage.CourseManageServlet']:{StudyCourse:'read',StudySchedule:'read',GaStudyAnswer:'read',
  WebUnPreStudy:'read',WebCtPreStudy:'read',TeacherPrestudySummary:'read'},
 [servlet+'coursemanage.TextBookManageServlet']:{getTextBookProgress:'read',getCourseBookReport:'read'},
 [servlet+'dailyzerotest.DailyZeroTestServlet']:{DtResultSearch:'read'},
 // DailyRecord: the page read, and the field writes that are GET requests.
 [servlet+'cct.tutor.DayRecordServlet']:{Main:'read',udtPrg:'write',udtHw:'write',udtMemo:'write',udtAttn:'write'},
 [servlet+'cct.tutor.AttendanceServlet']:{Main:'read'},
 [servlet+'cct.common.CourseCommonServlet']:{CismChartMain:'read',SetCismContentData:'write',SetCismChartData:'write',SetDailyTest:'write',SetHomeWorkRate:'write'},
 [servlet+'tutor.base.TestPageListServlet']:{UserBySearchTestResult:'read',UserByMain:'read'},
 [servlet+'common.TestPageShowExServlet']:{GetLectureList:'read'},
 // lmsRouteRegistry.ts legacyRoutes class these read; that class supersedes the legacy write candidates of
 // endpointCandidates.ts for getStudyResultSingleTestingSingleUser (specs/academy-mcp/direct-key-lookup.md R3).
 [servlet+'common.TestpageSelectExServlet']:{getStudyResultSingleTestingSingleUser:'read',getStudyResultSingleTestingMultiUser:'read',getWrongAnswerNoteResult:'read',getTestExamList:'read'},
 [servlet+'tutor.fa.TestPageListServlet']:{PupilSearch:'read'},
 [servlet+'tutor.na.TestPageListServlet']:{PupilSearch:'read'},
 [servlet+'tutor.fa.TestPageListGrpServlet']:{Main:'read'},
 [servlet+'tutor.diag.DiagnostManageServlet']:{TestPaperList:'read',TestResultList:'read'},
 [servlet+'tutor.etest.TestPoolServlet']:{SaveTestPool:'write',SaveLectureUseLog:'write'},
 [servlet+'tutor.na.TestPageRegServlet']:{NonePaper:'write'},
 '/common/etest_pool.jsp':{'':'write'},
};
/** The named route of one request: its path and operation. */
export function lmsRoute(url:string|URL,body?:unknown):{path:string;operation:string}{
 const parsed=new URL(String(url),'https://lms.invalid');
 const form=typeof body==='string'?new URLSearchParams(body):body instanceof URLSearchParams?body:new URLSearchParams();
 const operation=parsed.searchParams.get('p_process')??parsed.searchParams.get('reqCmd')??form.get('p_process')??form.get('reqCmd')??'';
 return {path:parsed.pathname,operation};
}
/** The static print archives of the StudyQ storage (specs/studyq-linux/core.md R20): `GET` of exactly
 * `https://storage.studyq.net/data/contents/{exam|examtext}/...-print.zip` is a read; the same path on
 * another host, or any other method, is not classed and so is write. */
const printStorageOrigin='https://storage.studyq.net';
const printArchivePath=/^\/data\/contents\/(?:exam\/d6-\d{3}\/d4-\d{2}\/d2-\d{2}\/ek-\d+\/\d+-print\.zip|examtext\/d4-\d{2}\/d2-\d{2}\/etk-\d+\/\d+-print\.zip)$/;
export function lmsRequestRole(url:string|URL,body?:unknown,method?:string):LmsRequestRole{
 const parsed=new URL(String(url),'https://lms.invalid');
 if(parsed.origin===printStorageOrigin&&!parsed.username&&!parsed.password&&!parsed.search&&!parsed.hash&&printArchivePath.test(parsed.pathname)&&(method??'GET').toUpperCase()==='GET')return 'read';
 const {path,operation}=lmsRoute(url,body);
 return routeRoles[path]?.[operation]??'write';
}
/** A fixture transport that answers `write_forbidden` (HTTP 403) to every request classed
 * write and passes the others to `answer`; `requests` records each request's route and class. */
export function writeForbiddenTransport(answer:(url:string,init?:RequestInit)=>Response|Promise<Response>){
 const requests:{route:string;role:LmsRequestRole}[]=[];
 const transport=async(url:string|URL,init?:RequestInit)=>{
  const {path,operation}=lmsRoute(url,init?.body),role=lmsRequestRole(url,init?.body,init?.method);
  requests.push({route:operation?path+' '+operation:path,role});
  if(role==='write')return new Response('write_forbidden',{status:403});
  return answer(String(url),init);
 };
 return {transport,requests,forbidden:()=>requests.filter(r=>r.role==='write')};
}
