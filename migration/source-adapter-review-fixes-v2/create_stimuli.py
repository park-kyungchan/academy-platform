"""New fictional stimuli, specified without reading any Rust output."""
import json,pathlib
base=pathlib.Path(__file__).resolve().parent
ref=base.parent/'source-adapter-reference-v1/corpus'
html=(ref/'presentation.html').read_text()
constructs={
 'template': 'function previewDBExam(a){return `x${a + "가😀"}y`;}',
 'arrow':'var previewDBExam = (a = 7, ...rest) => ({value:a, label:"새😀"});',
 'async':'var previewDBExam = async (a) => await Promise.resolve(a + 4);',
 'regexp-division':'function previewDBExam(a){return /ab+c/gi.test(a) ? a / 2 : /[\\/]/u.test(a);}',
 'optional':'function previewDBExam(a){return a?.b?.(3) ?? a?.["가"];}',
 'loops':'function previewDBExam(a){for(let i=0;i<3;i++){a+=i;} while(a<9){a++;} do {a--;} while(a>8); for(const x of [1,2]){a+=x;} for(const k in {x:1}){a+=k;} return a;}',
 'destructure':'function previewDBExam({x = "default", ...rest} = {}){let [a,,b=4,...tail]=[1,2,3]; return {...rest,x,a,b};}',
 'spread':'function previewDBExam(...a){return Math.max(...a,9)+[...a,2].length;}',
 'switch-try':'function previewDBExam(a){try {switch(a){case 1:return "one";default:throw "other";}}catch({message="none"}){return message;}finally{a=0;}}',
 'utf16':r'''/*😀*/ function previewDBExam(){return ["\uD800","\uDFFF","\u{1f600}","\x41","\u0042",`x${"\uD801"}`];}''',
 'nested':'function previewDBExam(){function dbFormatterExam(){return "inner";} return function(){return "outer";};}',
 'class2022':'function previewDBExam(){class X {#x=2;static y="yes"; static {this.y="ok";} get(){return this.#x;}} return new X();}',
 'computed':'var previewDBExam = function(){return ({["x"]:1, x:2, get y(){return "y";}, z(a){return a+3;}});};',
 'division-template':'function previewDBExam(a){return `${a / 3}${/x/.test(a)}`;}',
 'assignment':'previewDBExam = (a) => "unobserved assignment";',
 'malformed':'function previewDBExam(a){let x = ;return "no";}',
 'return-outside':'return previewDBExam("no");',
 'typescript':'function previewDBExam(a: number){return a;}',
 'jsx':'function previewDBExam(){return <div/>;}',
 'decorator':'@dec class X {} function previewDBExam(){return 1;}',
 'using':'function previewDBExam(){using x = resource;return 1;}',
 'import-options':'function previewDBExam(){return import("x",{with:{type:"json"}});}',
 'import-source':'function previewDBExam(){return import.source("x");}',
 'regex-v':'function previewDBExam(){return /[a]/v;}',
 'regex-duplicate':'function previewDBExam(){return /a/gg;}',
 'regex-invalid':'function previewDBExam(){return /(/;}',
 'regex-u-invalid':'function previewDBExam(){return /[z-a]/u;}',
 'hashbang':'#!/bin/env node\nfunction previewDBExam(){return 1;}',
}
cases=[{'id':k,'op':'presentation','html':html,'helpers':v} for k,v in constructs.items()]
for k,handler in {'return':'return previewDBExam(2);','arrow':'(()=>previewDBExam(3))();','optional':'api?.previewDBExam?.(4);','loop':'for(let x of [1]){previewDBExam(x);}','malformed':'return ( ;'}.items():
 cases.append({'id':'handler-'+k,'op':'presentation','html':'<button id="difficulty" onclick="'+handler.replace('"','&quot;')+'">난이도</button>','helpers':constructs['template']})
for k,script in {'dynamic':'$("#dbExamListGrid").jqGrid(config);','arrow':'$("#dbExamListGrid").jqGrid({colNames:["새😀"],colModel:[{name:"n",formatter:(a)=>`${a + 5}`} ]});','computed':'$("#dbExamListGrid").jqGrid({["colModel"]:[{["name"]:"n",formatter:obj[key]}]});','malformed-fallback':'var broken=; $("#dbExamListGrid").jqGrid({colModel:[{name:"n",formatter:function(a){return a+2;}}]}); var more=;'}.items():
 cases.append({'id':'grid-'+k,'op':'presentation','html':'😀<script>'+script+'</script>','helpers':constructs['template']})
locations=['c_login.jsp','../login@next?email=a@b','?next=a@b','/ordinary@path?q=@','https://dc.gang-a.kr:443/ordinary@path','//dc.gang-a.kr/next','/next#','/next#bad','https://user@dc.gang-a.kr/next','https://foreign.invalid/next','','/next']
(base/'STIMULI.json').write_text(json.dumps({'presentation':cases,'locations':locations},ensure_ascii=True,indent=2)+'\n')
