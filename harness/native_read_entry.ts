import {createAcademyReadPlan,executeAcademyRead} from '../src/lms/academyReadProtocol';

/** Executed only by the native browser, never by importing this entry in Bun. */
export async function runNativeAcademyRead(input:unknown) {
  const plan=createAcademyReadPlan(input);
  const authenticated=location.origin===plan.origin&&!document.querySelector('input[type=password]')
    &&!!document.body?.innerText.includes(plan.request.teacherLabel+'- [로그아웃]');
  return executeAcademyRead(plan,{origin:location.origin,authenticatedTeacherVisible:authenticated,
    principalKey:document.querySelector<HTMLInputElement>('[name=pri_no]')?.value,fetcher:(url,init)=>fetch(url,init)});
}
