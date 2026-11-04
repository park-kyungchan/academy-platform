// Regenerate inert, synthetic XML observations using the actual TS source export.
// From repository root with its Bun dependencies installed:
// bun crates/edu-academy-sources/tests/fixtures/capture-xml-document-boundaries.ts <source-root> <output-json>
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const sourceRoot=resolve(process.argv[2]??process.cwd());
const output=resolve(process.argv[3]??'crates/edu-academy-sources/tests/fixtures/xml-document-boundaries.json');
const {sourceDomParser}=await import(pathToFileURL(resolve(sourceRoot,'apps/academy/dc-gang-a/src/lms/academyHttpClient.ts')).href);
import {writeFile} from 'node:fs/promises';
const root='<root><record>한글 &amp; 😀</record></root>';
const cases:{id:string;source:string}[]=[{id:'bare',source:root}];
for(const [id,text] of [['space',' '],['xml-whitespace',' \t\r\n'],['decimal-space','&#32;'],['hex-space','&#x20;'],['decimal-tab','&#9;'],['hex-lf','&#xA;'],['decimal-cr','&#13;'],['named-entity','&amp;'],['non-xml-nbsp','\u00a0'],['raw-letter','x']]){
 for(const position of ['before','after'])cases.push({id:id+'-'+position,source:position==='before'?text+root:root+text});
}
cases.push({id:'reported-counterexample',source:'<root><record>ok</record></root>&#32;'},
 {id:'entity-before-with-raw-space',source:' \t&#32;'+root},
 {id:'entity-after-cr',source:root+'\r&#32;'},
 {id:'entity-after-crlf',source:root+'\r\n&#32;'},
 {id:'raw-word-before',source:'abc'+root},
 {id:'raw-word-after',source:root+'abc'},
 {id:'entity-after-raw-space',source:root+' \t&#32;'},
 {id:'entity-after-newline',source:root+'\n&#32;'},
 {id:'inside-root-whitespace-entity',source:'<root>&#32;<record>한글 &amp; 😀</record>&#9;</root>'},
 {id:'inside-record-whitespace-entity',source:'<root><record>&#32;한글 &amp; 😀&#9;</record></root>'},
 {id:'comments-around-root',source:'<!--before-->'+root+'<!--after-->'},
 {id:'processing-instructions-around-root',source:'<?before data?>'+root+'<?after data?>'});
const results=cases.map(input=>{
 try {const xml=sourceDomParser.parseFromString(input.source,'text/xml');return {...input,outcome:'returned',record:xml.querySelector('record')?.textContent};}
 catch(error){const e=error as Error&{code?:number};return {...input,outcome:'threw',name:e.name,message:e.message,code:e.code};}
});
await writeFile(output,JSON.stringify(results,null,2)+'\n');
console.log(JSON.stringify({cases:results.length,output}));
