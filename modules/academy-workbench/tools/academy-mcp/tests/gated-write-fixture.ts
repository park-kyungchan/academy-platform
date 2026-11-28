import './network-deny.ts';
import {appendFileSync,existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {McpServer} from '@modelcontextprotocol/server';
import {StdioServerTransport} from '@modelcontextprotocol/server/stdio';
import {installToolAudit} from '../src/tool-audit.ts';
import {ClinicPaperService,registerClinicPaperTools} from '../src/clinic-write-tools.ts';
import {fixtureLms,fixturePort,pageHashes} from './gated-write-fixture-lms.ts';

// A recorded synthetic dailyrecord endpoint for tests/gated-writes.test.ts: the real clinic tools and tool audit over the recorded
// synthetic LMS. Usage: bun gated-write-fixture.ts <ledger path> <base dir> <events file>.
// A file `inject-post-throw` in the base directory makes the write request fail after it is dispatched.
const [ledgerPath,base,events]=process.argv.slice(2) as [string,string,string];
appendFileSync(events,'start\n');
const lms=fixtureLms();
if(existsSync(resolve(base,'inject-post-throw')))lms.state.writeAnswer=()=>{throw Error('synthetic_transport_failure');};
const server=new McpServer({name:'fictional-gated-write',version:'1.0.0'});
installToolAudit(server,{endpoint:'dailyrecord'});
registerClinicPaperTools(server,new ClinicPaperService(fixturePort(lms),ledgerPath,{expectedWireHashes:pageHashes()}));
await server.connect(new StdioServerTransport());
