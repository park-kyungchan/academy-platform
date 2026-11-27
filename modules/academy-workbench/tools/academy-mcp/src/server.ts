import {installToolAudit} from './tool-audit.ts';
import {academyRoutingDescription} from './routing-context.ts';
import {runtimeIdentityConfiguration,registerRuntimeIdentity} from './runtime-identity.ts';
import {installStdioDiagnostics,createAcademyStdioTransport} from './stdio-diagnostics.ts';
import {withoutRetiredLearningArguments} from './dailyrecord-learning.ts';
import { McpServer } from '@modelcontextprotocol/server';
import { capabilitiesInputSchema, loadService } from './service.ts';
import { readOnlyAnnotations } from './contracts.ts';
import {toolResult as result} from './tool-result.ts';
import { repoRoot } from './paths.ts';
import {dailyRecordToolConfiguration,registerDailyRecordTools} from './dailyrecord-tools.ts';
import {DailyRecordService} from './dailyrecord.ts';
import {nativeDailyRecordConfiguration} from './native-dailyrecord-backend.ts';

const service = await loadService(repoRoot);
const server = new McpServer({ name: 'academy-operations', version: '0.1.0' });
installToolAudit(server,{endpoint:'server'});
const runtimeIdentity=await runtimeIdentityConfiguration(process.argv.slice(2));
const learning=withoutRetiredLearningArguments(runtimeIdentity.remainingArgs);
const configuration=await dailyRecordToolConfiguration(learning.remainingArgs);
// P3: read backend is injected once by the owner launch configuration, the
// same path as dailyrecord-server.ts. Without owner args the backend stays
// null and read tools return unavailable; runtimes only connect.
// The native triple belongs to the DailyRecord backend only. The item-label,
// saved-query and source-status tools of earlier releases are retired: a label
// argument that an installed launch still passes is accepted and ignored.
const hasNativeTriple=configuration.remainingArgs[0]==='--dailyrecord-native';
const native=await nativeDailyRecordConfiguration(hasNativeTriple?configuration.remainingArgs.slice(0,3):[],configuration.saveAdmission,configuration.saveStoreDir,configuration.saveRequestPath);
registerRuntimeIdentity(server,runtimeIdentity.identity);
registerDailyRecordTools(server,new DailyRecordService(native?.backend??null,configuration.parentSafeguard,Date.now,null,native?.backend??null));
if(native)process.once('exit',native.close);
server.registerTool('academy_capabilities', {
  title: 'Academy operational hierarchy and source contracts',
  description: academyRoutingDescription('academy_capabilities','Discover hierarchical academy workflows, page the complete pinned source inventory (limit 1–100, default 10), or inspect one exact source contract. Source metadata is not an executable operation. No source requests.'),
  inputSchema: capabilitiesInputSchema,
  annotations: readOnlyAnnotations,
}, input => result(service.capabilities(input)));
installStdioDiagnostics(server.server);
await server.connect(createAcademyStdioTransport());
