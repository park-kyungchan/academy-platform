import { resolve } from 'node:path';
import { readLiteralKey, readLiteralName } from '../lms/academyReadProtocol';
import type { SourceDiscoveryRequest } from './nativeBookSource';
import type { Acquisition } from './refresh';

/** Narrow native Python compatibility bridge. Native browser/vault keep secrets and origin checks; no cookies leave them. */
const nativeBridge = String.raw`import json,sys,time
from tools.browser_use_cli import browser_exec
from tools.browser_vault_tool import browser_vault_list,browser_vault_fill
request=json.loads(sys.stdin.read())
def run(code,timeout=60):
    return json.loads(browser_exec(code,session='dc-source-service',timeout_s=timeout))
def emit(value):
    sys.stdout.write(json.dumps(value,ensure_ascii=False))
initial=run("ensure_real_tab(); goto_url('https://dc.gang-a.kr/c_login.jsp'); wait_for_load(); print('SOURCE_PAGE_READY')")
if not initial.get('success'):
    emit({'status':'blocked','reason':'native_browser_unavailable'});sys.exit(0)
items=json.loads(browser_vault_list()).get('items',[])
logins=[item for item in items if item.get('kind')=='login' and item.get('origin')=='https://dc.gang-a.kr' and item.get('available')]
if len(logins)!=1:
    emit({'status':'blocked','reason':'origin_bound_login_required'});sys.exit(0)
identifier=logins[0].get('identifier')
if not identifier:
    emit({'status':'blocked','reason':'origin_bound_login_required'});sys.exit(0)
filled=run("fill_input('#webUserId',"+json.dumps(identifier)+"); print('IDENTIFIER_SET')")
password=json.loads(browser_vault_fill(logins[0]['handle']))
if not filled.get('success') or not password.get('success'):
    emit({'status':'blocked','reason':'native_vault_authentication_required'});sys.exit(0)
# Keep executable glue constant; only reviewed source input and bundled function cross this seam.
code = "js(\"document.querySelector('.webSend').click()\")\nwait_for_load()\nimport time\nfor _ in range(24):\n    if js("+repr("!!document.body?.innerText.includes("+json.dumps(request['teacherLabel']+'- [로그아웃]')+")")+"): break\n    time.sleep(0.25)\n"
expression=request['bundle']+";collectNativeBookDiscovery("+json.dumps(request['scope'],ensure_ascii=False)+")"
code += "r=cdp('Runtime.evaluate',expression="+repr(expression)+",returnByValue=True,awaitPromise=True,_response_timeout=180)\nif r.get('exceptionDetails'): raise RuntimeError('source_read_refused')\nprint('DC_SOURCE_VALUE='+json.dumps(r['result']['value'],ensure_ascii=False))\n"
result=run(code,240)
if not result.get('success'):
    emit({'status':'blocked','reason':'source_authentication_or_contract_refused'});sys.exit(0)
line=next((line for line in result.get('output','').splitlines() if line.startswith('DC_SOURCE_VALUE=')),None)
if not line:
    emit({'status':'blocked','reason':'source_result_not_retained'});sys.exit(0)
emit({'status':'observed','data':json.loads(line.split('=',1)[1])})
`;
let busy=false;
export async function nativeBookAcquirer(input:SourceDiscoveryRequest):Promise<Acquisition>{
  if(!Array.isArray(input.studentNames)||input.studentNames.length<1||input.studentNames.length>64||new Set(input.studentNames).size!==input.studentNames.length)throw Error('invalid_source_scope');
  readLiteralName(input.teacherLabel);readLiteralKey(input.teacherKey);readLiteralKey(input.collegeKey);input.studentNames.forEach(readLiteralName);
  if(process.env.HERMES_HOME!=='/opt/data/profiles/dc-gang-a-dev')throw Error('active_profile_required');
  if(busy)return {status:'blocked',reason:'source_operation_in_progress'};
  busy=true;
  try{
    const build=await Bun.build({entrypoints:[resolve(import.meta.dir,'nativeBookSource.ts')],target:'browser',format:'esm',minify:false});
    if(!build.success)return {status:'blocked',reason:'source_bundle_failed'};
    const code=await build.outputs[0]!.text(),boundary=code.lastIndexOf('\nexport {');
    if(boundary<0||!code.slice(boundary).includes('collectNativeBookDiscovery'))return {status:'blocked',reason:'source_bundle_export_invalid'};
    const bundle=code.slice(0,boundary);
    const process=Bun.spawn(['/opt/hermes/.venv/bin/python','-B','-c',nativeBridge],{stdin:'pipe',stdout:'pipe',stderr:'pipe',env:{...globalThis.process.env,PYTHONPATH:'/opt/hermes'}});
    process.stdin.write(JSON.stringify({scope:input,teacherLabel:input.teacherLabel,bundle}));process.stdin.end();
    const [stdout,exit]=await Promise.all([new Response(process.stdout).text(),process.exited,new Response(process.stderr).text()]);
    if(exit!==0||stdout.length>2000000)return {status:'blocked',reason:'native_connector_failed'};
    const result:Acquisition=JSON.parse(stdout);
    return result;
  }catch{return {status:'blocked',reason:'native_connector_unavailable'};}
  finally{busy=false;}
}
