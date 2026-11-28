import './network-deny.ts';
import {appendFileSync,existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {McpServer} from '@modelcontextprotocol/server';
import {StdioServerTransport} from '@modelcontextprotocol/server/stdio';
import {installToolAudit} from '../src/tool-audit.ts';
import {ClinicPaperService,registerClinicPaperTools} from '../src/clinic-write-tools.ts';
import {ExamPaperService,registerExamPaperTools} from '../src/exam-write-tools.ts';
import {fixtureLms,fixturePort,pageHashes} from './gated-write-fixture-lms.ts';
import {examFixturePort,fixtureExamLms,pageHashes as examPageHashes} from './exam-write-fixture-lms.ts';

// A recorded synthetic dailyrecord endpoint for tests/gated-writes.test.ts: the real clinic tools and tool audit over the recorded
// synthetic LMS. Usage: bun gated-write-fixture.ts <ledger path> <base dir> <events file>.
// A file `inject-post-throw` in the base directory makes the clinic write request fail after it is dispatched, and `inject-hang-throw` the exam assignment write.
// Each call is a new process with a new recorded LMS: a review and its save see no state of each other beyond the ledger.
const [ledgerPath,base,events]=process.argv.slice(2) as [string,string,string];
appendFileSync(events,'start\n');
const lms=fixtureLms();
if(existsSync(resolve(base,'inject-post-throw')))lms.state.writeAnswer=()=>{throw Error('synthetic_transport_failure');};
const examLms=fixtureExamLms();
if(existsSync(resolve(base,'inject-hang-throw')))examLms.state.onRequest=r=>{if(r.route.includes('TestHangServlet Save'))throw Error('synthetic_transport_failure');};
const server=new McpServer({name:'fictional-gated-write',version:'1.0.0'});
installToolAudit(server,{endpoint:'dailyrecord'});
registerClinicPaperTools(server,new ClinicPaperService(fixturePort(lms),ledgerPath,{expectedWireHashes:pageHashes()}));
registerExamPaperTools(server,new ExamPaperService(examFixturePort(examLms),ledgerPath,{expectedWireHashes:examPageHashes()}));
await server.connect(new StdioServerTransport());
