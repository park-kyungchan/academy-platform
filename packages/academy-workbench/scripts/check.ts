const checks = [ ['typecheck', ['run','typecheck']], ['test', ['test']], ['build', ['run','build']], ['validate', ['run','validate']], ['demo', ['run','demo']] ] as const;
const results = [];
for(const [name,args] of checks) {
  const child=Bun.spawn(['bun',...args],{stdout:'pipe',stderr:'pipe'});
  const [stdout,stderr,exitCode]=await Promise.all([new Response(child.stdout).text(),new Response(child.stderr).text(),child.exited]);
  results.push({name,command:['bun',...args].join(' '),exitCode,output:stdout+stderr});
  console.log(`${name}: ${exitCode===0?'PASS':'FAIL'} (exit ${exitCode})`);
}
await Bun.write('docs/book-flow-validation-results.json',JSON.stringify({checkedAt:new Date().toISOString(),bun:Bun.version,platform:process.platform,results},null,2)+'\n');
if(results.some(r=>r.exitCode!==0))process.exit(1);
export {};
