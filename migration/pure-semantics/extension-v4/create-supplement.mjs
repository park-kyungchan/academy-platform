// Forward-only additions after run01. Earlier baseline/cases/expectations intact.
import {readFileSync,writeFileSync,readdirSync,realpathSync,statSync} from 'node:fs';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
const here=dirname(fileURLToPath(import.meta.url)),root=resolve(here,'../../..'),sha=b=>createHash('sha256').update(b).digest('hex'),json=p=>JSON.parse(readFileSync(p,'utf8'));
const cases=[],add=(id,input,script='baseline.ts')=>cases.push({id,input,script,contract:'S08/EDU-17',focus:'additive source export/boundary coverage'});
const ports={now:'2040-10-15T09:00:00.000Z'};
const parse=(id,extra={})=>add('parser-'+id,{ports,...extra},'parser-baseline.ts');
parse('valid');parse('script-inert',{replacements:[['<script>','<script>throw Error("must_not_execute");']]});
parse('date-rejected',{date:'2040-02-30'});parse('body-bound',{html:'x'.repeat(2000001)});
parse('login',{replacements:[['<h1>','<input type="password"><h1>']]});
for(const [id,from,to] of [['heading','주간 교무회의 CISM 작성','Other'],['date','value="2040-10-15"','value="2040-10-16"'],['teacherkey','value="90001"','value="99999"'],['teacherlabel','value="Fictional Teacher"','value="Other"'],['command','CismChartMain','Other'],['wire','reqCmd=SetCismChartData','reqCmd=WrongOperation'],['period','2040-42 주차 (','2040-42 주차 (1'],['narrative-handler',"javascript:onTextareaFocusOut(this,'i_1');",'Other'],['metric-handler',"setCismData('think_note',0,90002,90003,1);","setCismData('think_note',0,0,90003,1);"],['options','<option value="3">','<option value="4">'],['narrative-duplicate','</table>','</table><textarea id="i_1" name="i_1"></textarea>'],['metric-duplicate','</table>','</table><input id="think_note_0" onfocusout="setCismData(\'think_note\',0,90002,90003,1);">']])parse(id,{replacements:[[from,to]]});
parse('teacher-null',{teacher:null}); // Source caller default in adapter is explicit; no invented source null claim.
const js=t=>({$js:t});
for(const fn of ['parseCalendarDate','assertDate'])for(const v of ['0000-01-01','2000-02-29','2100-02-29','9999-12-31','2040-01-16\n',null,js('undefined')])add('direct-'+fn+'-'+cases.length,{op:'learning',fn,args:[v]});
for(const fn of ['canonicalJson','digest'])for(const v of [{b:1,a:2},{a:js('undefined')},js('cycle')])add('direct-'+fn+'-'+cases.length,{op:'learning',fn,args:[v]});
for(const v of [['12:30'],['12:30','13:00'],'12:30\n'])add('time-coercion-'+cases.length,{op:'learning',fn:'assertTime',args:[v]});
const request={runtime:'runtime:synthetic',requestRef:'request:synthetic',requestTextSha256:'a'.repeat(64),currentUserEventRef:'event:synthetic'},text=JSON.stringify(request);
add('live-evidence-boundaries',{op:'live_evidence',steps:[{bytes:text+' '.repeat(16384-text.length)},{bytes:text+' '.repeat(16385-text.length)},{bytes:'\ufeff'+text},{bytes:'{"runtime":"wrong",'+text.slice(1)},{bytes:'null'},{missing:true}]});
const main=json(join(here,'cases.json')),base=main.find(c=>c.id==='review-instant-0').input;
for(const [id,stamp]of [['one-fraction','2040-01-16T18:00:00.5+09:00'],['two-fraction','2040-01-16T03:30:00.12-05:30'],['nine-fraction','2040-01-16T09:00:00.123456789Z']])add('review-valid-'+id,{...base,observation:{...base.observation,observedAt:stamp},ports:{...base.ports,nowEpochMs:Date.parse(stamp)}});
const domRoot=realpathSync('/home/palantirkc/projects/education-platform/apps/academy/dc-gang-a/node_modules/jsdom'),packages=new Map();
function packageRoot(entry){let d=dirname(entry);while(!statSync(join(d,'package.json'),{throwIfNoEntry:false})?.isFile())d=dirname(d);return realpathSync(d);}
function visit(d){if(packages.has(d))return;const p=json(join(d,'package.json'));packages.set(d,p);const req=createRequire(join(d,'package.json'));for(const name of Object.keys(p.dependencies??{}))visit(packageRoot(req.resolve(name)));}
visit(domRoot);
const files={};function walk(d){for(const e of readdirSync(d,{withFileTypes:true})){if(e.name==='node_modules'||e.name==='.git')continue;const p=join(d,e.name);if(e.isDirectory())walk(p);else if(e.isFile())files[p]=sha(readFileSync(p));}}
for(const d of packages.keys())walk(d);
const sourceExtra=['apps/academy/dc-gang-a/tests/lms/fixtures/cism-handlers.js.txt','apps/academy/dc-gang-a/tests/lms/cismWeeklyProtocol.test.ts'];
const pin={schema:'s08.extension-supplement-pins/4',domRoot,packages:[...packages].map(([path,p])=>({path,name:p.name,version:p.version})),dependencyFiles:files,sourceExtra:Object.fromEntries(sourceExtra.map(p=>{const b=readFileSync(join(root,p)),g=spawnSync('git',['show','cc6e81f1be2fcb56f011041e3d606e21edf76da3:'+p],{cwd:root});if(g.status!==0||!g.stdout.equals(b))throw Error('original_source_changed:'+p);return [p,sha(b)];})),casesSha256:sha(JSON.stringify(cases,null,2)+'\n'),clockProvider:'new Date() and Date.now overridden before source import; argument constructors/parse/UTC retain pinned native engine'};
writeFileSync(join(here,'supplement-cases.json'),JSON.stringify(cases,null,2)+'\n',{flag:'wx'});writeFileSync(join(here,'supplement-pins.json'),JSON.stringify(pin,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({cases:cases.length,packages:packages.size,dependencyFiles:Object.keys(files).length}));
