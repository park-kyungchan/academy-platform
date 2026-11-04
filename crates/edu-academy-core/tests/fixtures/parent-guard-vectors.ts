// Reproduce all regex vectors in parentDraftAdmission.test.ts using the actual
// TS source as oracle; Rust consumes the checked-in result without needing Bun.
import {readFileSync, writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {parentInternalLanguage} from '../../../../apps/academy/dc-gang-a/src/contracts/parentInternalLanguage';
const root = new URL('../../../../', import.meta.url);
const files = ['apps/academy/dc-gang-a/src/contracts/parentInternalLanguage.ts',
  'apps/academy/dc-gang-a/tests/academy/parentDraftAdmission.test.ts',
  'apps/academy/dc-gang-a/research/textbooks/edition-catalog-20261028-v1.json',
  'apps/academy/dc-gang-a/research/textbooks/edition-catalog-20261028-v2.json',
  'apps/academy/dc-gang-a/docs/daily-record-authoring-policy.md'];
const cases: {text: string, excluded: boolean}[] = [];
function add(text: string, expected: boolean) {
  const excluded = parentInternalLanguage.test(text);
  if(excluded !== expected) throw Error(`TS oracle mismatch: ${text}`);
  cases.push({text, excluded});
}
const codes = new Set(['1P6A','1U0T']);
for(const version of ['v1','v2']) {
  const catalog=JSON.parse(readFileSync(new URL(`apps/academy/dc-gang-a/research/textbooks/edition-catalog-20261028-${version}.json`,root),'utf8'));
  for(const book of catalog.books) { codes.add(book.identity.printedCoverCode); add(`- ${book.identity.brand} ${book.identity.printedCoverCode} 준비`,true); }
}
const policy=readFileSync(new URL(files[4]!,root),'utf8');
for(const code of policy.match(/(?<![A-Za-z0-9])[1-9][A-Z][0-9][A-Z](?![A-Za-z0-9])/g)??[])codes.add(code);
if(codes.size!==10)throw Error('catalog/policy drift');
for(const code of codes)for(const value of [code,code.toLowerCase(),code[0]+code[1]!.toLowerCase()+code.slice(2)]) {
  add(`- 가우스 5-1-2 (${value}) 준비`,true);add(`- 가우스 ${value} 준비`,true);
}
for(const text of ['가우스 시리즈2 v1(1SAG) 준비','가우스 1-1-2 (1P6A) 준비','가우스 (1U0T) 준비','가우스 5-1-2 [1P6A] 준비','- 가우스 1-1-2 (1P6A) 준비','- 다빈치 1SJ2 준비','- 가우스플러스 (1U0T) 준비','- 교재 1P6A 준비','- 가우스 초5-1 1SAG 준비','- 교과서:1U0T 준비','- 시리즈 (1P6A) 준비','- 교재v3(1P6A) 준비','Teacher REVIEW','Ungraded','NOT COMPLETED','Insufficient TIME','VERSION:3','V 3','(1p6A)'])add(text,true);
for(const code of ['2Q4Z','9Z0Q','2X3Y']) {add(`- 가우스 (${code}) 준비`,true);add(`- 가우스 ${code} 준비`,true);}
for(const text of ['- 단항식 2x3y를 간단히 하기','- 2x3y의 계산 연습','- (2x3y)의 계산 연습','- 다항식 3abc 계산','- x2y3를 간단히 하기','- 단항식 2X3Y를 간단히 하기','- 가우스 5-1-2 준비','- 2026-11-02까지 12문항 풀기'])add(text,false);
const wrappers=[['',''],['(',')'],['[',']'],['{','}'],['「','」'],['（','）'],['"','"'],["'","'"],['<','>'],['【','】']];
for(const code of codes)for(const value of [code,code.toLowerCase()])for(const [open,close] of wrappers) {
  add(`- 가우스 5-1-2 ${open}${value}${close} 준비`,true);add(`- ${open}${value}${close} 준비`,true);
}
for(const value of ['2x3y','3abc','x2y3','[2x3y]','(2x3y)','단항식 2X3Y'])add(`- ${value}를 간단히 하기`,false);
// Additional boundary/whitespace/case regressions.
for(const text of ['A1SAG','1SAG2','a1p6ab','가우스 2q4z','교재 2Q4Za','가우스 2Q4Z0','v\u00853'])add(text,false);
for(const text of ['한글1sag한글','1SAG_','교재명:#1P6A','문제집:2Q4Z','가우스 초5-1-2 2Q4Z','v\ufeff3','교사 최종 완료판정 대기'])add(text,true);
const output=JSON.stringify({sources:files.map(path=>({path,sha256:createHash('sha256').update(readFileSync(new URL(path,root))).digest('hex')})),cases},null,2)+'\n';
const target=new URL('./parent-guard-vectors.json',import.meta.url);
if(process.argv.includes('--check')){if(readFileSync(target,'utf8')!==output)throw Error('regenerate parent guard vectors');}
else writeFileSync(target,output);
console.log(`${cases.length} TS parent guard vectors verified`);
