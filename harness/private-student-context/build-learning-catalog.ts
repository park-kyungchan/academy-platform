import {readFileSync,writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';

// Explicitly bounded imported corpus; no runtime, config, credentials, discovery or live reads.
const root=resolve(import.meta.dir,'../..'),pkg=resolve(root,'context/private-students/2026-10-21');
const original='history/academy-handoff-20261021/';
const hash=(v:string|Buffer)=>createHash('sha256').update(v).digest('hex');
const prior=JSON.parse(readFileSync(resolve(root,'context/private-students/2026-10-16/history/actual-catalog-sections.json'),'utf8'));
const names=['유지연','신지우','박세은','이현승','이루한'];
const studentRefs=Object.fromEntries(names.map(name=>{const refs=new Set<string>(prior.rows.filter((r:any)=>r.student===name&&typeof r.row?.id==='string'&&/stu_[a-f0-9]+/.test(r.row.id)).map((r:any)=>r.row.id.match(/stu_[a-f0-9]+/)[0]));if(refs.size===0){const page=readFileSync(resolve(root,`context/private-students/2026-10-16/space/${name}.page.json`),'utf8');for(const match of page.matchAll(/\"studentId\"\s*:\s*\"(student_[a-f0-9]+)\"/g))refs.add(match[1]!);}if(refs.size!==1)throw Error('retained_student_identity_ambiguous');return [name,[...refs][0]!];}));
const corrections={schemaVersion:'academy.scoped-mapping-corrections/1',recordedOn:'2026-10-21',source:'current direct user interview replies',changes:[
 {questionItemId:'["request_user_input_async","call_e21c07736e0b473cb2346bde1641c6cd",0]',scope:'permanent parent presentation',answer:'단원별 한 줄에 섹션을 모두 표시',meaning:'Group all sections only within the same student, edition, unit, activity and deadline.'},
 {questionItemId:'["request_user_input_async","call_e21c07736e0b473cb2346bde1641c6cd",1]',studentRef:studentRefs['이현승'],answer:'초6-1 시리즈1이다. 초6-2 시리즈1은 잘못 타이핑한 것.',meaning:'Current Davinci book is elementary grade6 term1 series1. Preserve raw term2 typo; native-code/TOC equivalence is not independently verified.'},
 {questionItemId:'["request_user_input_async","call_e21c07736e0b473cb2346bde1641c6cd",2]',scope:'permanent notation grammar',answer:'가우스 5-1-2: 5학년1학기 시리즈2. 가우스 5-1-2-4: 5학년1학기 대단원2 소단원4. 모호하면 항상 질문.',meaning:'Never infer series from four-part curriculum notation; unknown notation also requires an interview.'},
 {questionItemId:'["request_user_input_async","call_799c28de2f8a4ced910b47989d912224",0]',studentRef:studentRefs['박세은'],answer:'가우스 5학년2학기 시리즈1 대단원2에 소단원1(5-2-2-1).',meaning:'The already-completed 2-1 실력다지기·실력올리기 report is bound here. Retain teacher-reported labels without claiming printed/source-label equivalence.',completion:'teacher_confirmed_complete_preserved'}
],effects:{nativeBookRegistrationChanged:false,dailyRecordRewritten:false,parentSends:0}};
writeFileSync(resolve(pkg,'views/mapping-corrections.json'),JSON.stringify(corrections,null,2)+'\n');
const correctionRef='views/mapping-corrections.json';
const books:any[]=[],observations:any[]=[],coverage:any[]=[];
let excludedStatusColumns=0,sourceNodes=0;
for(const [i,name] of names.entries()){
 const file=original+`${11+i}-${name}.md`,bytes=readFileSync(resolve(pkg,file)),lines=bytes.toString('utf8').split('\n');
 let book:any=null,unit:any=null;const startBooks=books.length,startNodes=sourceNodes;
 for(const [index,line] of lines.entries()){
  if(line.startsWith('### ')){
   book=null;unit=null;
   if(!/^### (?:현재 교재|별도 교재|구교재 이력|추가 식별 교재) · /.test(line))continue;
   const raw=line.replace(/^### [^·]+ · /,'');const m=/(초|중)(\d+)-(\d+)/.exec(raw),brand=raw.includes('가우스 플러스')?'가우스 플러스':raw.includes('가우스')?'가우스':raw.includes('다빈치')?'다빈치':null;
   if(!brand)throw Error('unhandled_book_brand');
   const volume=/(?:시리즈\s*(\d+)|(\d+)권)/.exec(raw),v=/\bv(\d+)\b/.exec(raw),ref=`${file}:${index+1}:sha256:${hash(bytes)}`;
   const corrected=name==='이현승'&&brand==='다빈치'&&m?.[2]==='6';
   const grade=m?Number(m[2]):null,term=corrected?1:m?Number(m[3]):null,school=m?.[1]==='초'?'elementary':m?.[1]==='중'?'middle':null;
   const seriesKind=school==='middle'&&brand==='다빈치'?'not_applicable':volume?'numbered':'unknown';
   book={studentRef:studentRefs[name],bookRef:`retained-book:${hash(name+'|'+raw).slice(0,24)}`,label:brand+' '+(school==='elementary'?'초':school==='middle'?'중':'')+(grade===null?'학년 미확정':`${grade}-${term}`)+(seriesKind==='numbered'?` 시리즈${Number(volume![1]??volume![2])}`:'')+(v?` v${v[1]}`:''),brand,schoolLevel:school,grade,term,series:seriesKind==='numbered'?Number(volume![1]??volume![2]):null,seriesKind,edition:v?`v${v[1]}`:null,bindingState:corrected||m&&seriesKind!=='unknown'?'confirmed':'unresolved',units:[],evidenceRefs:[ref,...corrected?[correctionRef+'#changes/1']:[]]};
   books.push(book);observations.push({bookRef:book.bookRef,rawHeader:raw,sourceRef:ref,currentBookIdentity:corrected?'teacher_corrected_term1':'retained_source_report',nativeCodeQualification:corrected?'not_independently_verified_after_typo_correction':'retained_only',printedTocCompleteness:'unknown',sectionAliasCrosswalk:'unknown'});continue;
  }
  if(!book)continue;
  if(line.startsWith('#### ')){
   sourceNodes++;const heading=line.slice(5),minor=/^(\d+-\d+)\s+(.+)$/.exec(heading),major=/^(\d+)단원\s+(.+)$/.exec(heading),parsed=minor??major;
   unit={unitRef:`retained-unit:${hash(book.bookRef+'|'+index+'|'+heading).slice(0,24)}`,code:parsed?.[1]??heading,title:parsed?.[2]??heading,kind:minor?'minor':major?'chapter_assessment':'unknown',sections:[],evidenceRefs:[`${file}:${index+1}:sha256:${hash(bytes)}`]};book.units.push(unit);continue;
  }
  if(unit&&line.startsWith('- 섹션별 서버 표시(보존 스냅샷): ')){
   const columns=line.slice(line.indexOf(': ')+2).split('; ');
   for(const [column,value] of columns.entries()){
    const label=value.slice(0,value.lastIndexOf(':')).trim();
    if(!label)throw Error('source_section_label_missing');
    if(/평가클리닉|오답재학습|^숙제$/.test(label)){excludedStatusColumns++;continue;}
    unit.sections.push({sectionRef:`retained-section:${hash(unit.unitRef+'|'+column+'|'+label).slice(0,24)}`,label,basis:'source_catalog',evidenceRefs:[`${file}:${index+1}:column:${column+1}:sha256:${hash(bytes)}`]});
   }
  }
 }
 coverage.push({studentRef:studentRefs[name],sourceFile:file,sourceSha256:hash(bytes),bookCount:books.length-startBooks,unitOccurrences:sourceNodes-startNodes,printedTocComplete:false,missingIsIncomplete:false});
}
// The teacher resolves this one exact completed activity, not all book-wide section aliases.
const saeeunBook=books.filter(b=>b.studentRef===studentRefs['박세은']&&b.brand==='가우스'&&b.grade===5&&b.term===2&&b.series===1);
if(saeeunBook.length!==1)throw Error('teacher_corrected_book_ambiguous');
const saeeunUnit=saeeunBook[0].units.filter((u:any)=>u.code==='2-1');if(saeeunUnit.length!==1)throw Error('teacher_corrected_unit_ambiguous');
for(const label of ['실력다지기','실력올리기'])saeeunUnit[0].sections.push({sectionRef:`teacher-report:${hash(studentRefs['박세은']+'|5-2-2-1|'+label).slice(0,24)}`,label,basis:'teacher_report',evidenceRefs:[correctionRef+'#changes/3']});
const catalog={schemaVersion:'academy.learning-catalog/1',coverage:'observed_only',books};
writeFileSync(resolve(pkg,'views/learning-catalog.json'),JSON.stringify(catalog,null,2)+'\n');
const audit={schemaVersion:'academy.mapping-coverage/1',scope:'All recognized book sections and unit occurrences in the five supplied student handoff documents; historical catalogs retained separately.',sourceUnitOccurrences:sourceNodes,bookOccurrences:books.length,sectionOccurrences:books.flatMap(b=>b.units.flatMap((u:any)=>u.sections)).length,excludedStatusColumns,priorCatalogRows:prior.rowCount,priorCatalogUsedFor:'Existing retained student identity only; not promoted to current assignments or silently merged with newer editions.',students:coverage,books:observations,unknowns:['Printed full TOC coverage and page boundaries are not fully verified.','Server-to-printed-section aliases are not automatically equivalent.','Gauss Plus level/series and section catalog need scoped confirmation.','The corrected elementary6-1 Davinci book identity is teacher-confirmed; native code and detailed TOC equivalence remain independently unverified.'],stateRules:{catalogPresenceIsAssignment:false,missingIsIncomplete:false,teacherCompletionReopened:false,distinctAttemptsMerged:false},effects:{liveReads:0,sourceWrites:0,parentSends:0}};
writeFileSync(resolve(pkg,'views/mapping-coverage.json'),JSON.stringify(audit,null,2)+'\n');
const entryPath=resolve(pkg,'views/read-entry.json'),entry=JSON.parse(readFileSync(entryPath,'utf8'));
entry.currentMappingOverlay=['views/mapping-corrections.json','views/learning-catalog.json','views/mapping-coverage.json'];
entry.parentFormat.sectionGrouping='One line per exact student/book edition/unit/activity/deadline; show all selected sections.';
entry.mappingAmbiguity='Always interview; three parts are grade/term/series, four parts are grade/term/major/minor. Never infer series from major.';
writeFileSync(entryPath,JSON.stringify(entry,null,2)+'\n');
const manifestPath=resolve(pkg,'manifest.json'),manifest=JSON.parse(readFileSync(manifestPath,'utf8'));
for(const path of ['views/read-entry.json','views/mapping-corrections.json','views/learning-catalog.json','views/mapping-coverage.json']){
 const bytes=readFileSync(resolve(pkg,path)),artifact={path,sha256:hash(bytes),bytes:bytes.length,provenanceKind:'derived_view',sourceReferences:[original+'01-mapping-rules.md',original+'02-book-features-and-workflow.md','current scoped teacher interview'],coverage:'partial'};
 const old=manifest.artifacts.findIndex((a:any)=>a.path===path);if(old>=0)manifest.artifacts[old]=artifact;else manifest.artifacts.push(artifact);
}
writeFileSync(manifestPath,JSON.stringify(manifest,null,2)+'\n');
console.log(JSON.stringify({sourceUnitOccurrences:sourceNodes,bookOccurrences:books.length,sectionOccurrences:audit.sectionOccurrences,excludedStatusColumns,priorCatalogRows:prior.rowCount,liveReads:0,sourceWrites:0,parentSends:0}));
