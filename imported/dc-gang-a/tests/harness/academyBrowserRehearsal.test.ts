import {describe,expect,test} from 'bun:test';
import {resolve} from 'node:path';
const repo=resolve(import.meta.dir,'../..');
const run=(args:string[])=>Bun.spawnSync({cmd:[process.execPath,'--no-env-file','harness/academy_browser_rehearsal.ts',...args],cwd:repo,env:{...process.env,HERMES_HOME:'/opt/data/profiles/dc-gang-a-dev'},stdout:'pipe',stderr:'pipe'});
describe('synthetic-only browser rehearsal admission',()=>{
 test('advertises the opt-in synthetic PostgreSQL observation lane without making it default',()=>{
  const result=run(['--help']);expect(result.exitCode).toBe(0);
  const help=JSON.parse(new TextDecoder().decode(result.stdout));
  expect(help.teacherObservationOption).toBe('--teacher-observation-test-socket');
  expect(help.production).toBe(false);
 });
 test('refuses a production or foreign socket before opening PostgreSQL',()=>{
  const result=run(['--synthetic-only','--output-dir','/opt/data/profiles/dc-gang-a-dev/cache/scratch/denied-rehearsal','--teacher-observation-test-socket','/opt/data/profiles/dc-gang-a-dev/academy/socket']);
  expect(result.exitCode).not.toBe(0);
  expect(new TextDecoder().decode(result.stderr)).toContain('synthetic_test_socket_required');
 });
});
