import {installToolAudit} from './tool-audit.ts';
import {registerCismWeeklyTools} from './cism-weekly-tools.ts';
import {runtimeIdentityConfiguration,registerRuntimeIdentity} from './runtime-identity.ts';
import {installStdioDiagnostics,createAcademyStdioTransport} from './stdio-diagnostics.ts';
import {withoutRetiredLearningArguments} from './dailyrecord-learning.ts';
import {McpServer} from '@modelcontextprotocol/server';
import {DailyRecordService} from './dailyrecord.ts';
import {registerTextbookSourceTools} from './textbook-source-tools.ts';
import {textbookPdfReaderConfiguration} from './textbook-pdf-reader.ts';
import {dailyRecordToolConfiguration,registerDailyRecordTools,registerOfficialReportPdfTool,registerPaperPrintPdfTool} from './dailyrecord-tools.ts';
import {LmsScheduleService,registerLmsScheduleTool} from './lms-schedule-tools.ts';
import {ClinicPaperService,registerClinicPaperTools} from './clinic-write-tools.ts';
import {ExamPaperService,registerExamPaperTools} from './exam-write-tools.ts';
import {gatedWriteLedgerConfiguration} from '../../../../../apps/academy/dc-gang-a/src/lms/gatedWriteLedger';
import {historicalBindingReaderConfiguration} from './historical-binding-reader.ts';
import {pcExecutorConfiguration} from './pc-executor.ts';
import {pcPaperExecutor} from './pc-paper-executor.ts';
import {OfficialReportPdfService} from './official-report-pdf.ts';
import {PaperPrintPdfService,withoutRetiredStudyqRunnerArguments} from './paper-print-pdf.ts';
import {join} from 'node:path';
import {nativeDailyRecordConfiguration,nativeOwnerArtifactsRoot} from './native-dailyrecord-backend.ts';
import {createDailyRecordWorkflow,dailyRecordWorkflowConfiguration} from './dailyrecord-workflow.ts';

const runtimeIdentity=await runtimeIdentityConfiguration(process.argv.slice(2));
// --gated-write-ledger <absPath> (or ACADEMY_GATED_WRITE_LEDGER_PATH): the private attempt ledger of the gated writes; with neither, the gated saves refuse ledger_unavailable.
const gatedLedger=gatedWriteLedgerConfiguration(runtimeIdentity.remainingArgs);
const textbookReader=await textbookPdfReaderConfiguration(gatedLedger.remainingArgs);
// --memory-bindings <memoryBin> <storePath>; the retired --studyq-runner and its five words are dropped unread.
const memoryBindings=historicalBindingReaderConfiguration(textbookReader.remainingArgs);
const studyq=withoutRetiredStudyqRunnerArguments(memoryBindings.remainingArgs);
// --pc-executor <user@host> <identityFile> <knownHostsFile> <exeDir>: the official report PDFs are made by StudyQReport.exe
// on the user's PC (specs/academy-mcp/pc-executor.md), the paper PDFs by StudyQPdfManager.exe and ETestMaker.exe of the same
// install (specs/academy-mcp/pc-paper-executor.md); without it every report and paper answers pc_executor_not_configured.
const officialReportsRoot=join(nativeOwnerArtifactsRoot,'official-reports');
const pc=await pcExecutorConfiguration(studyq.remainingArgs,{lockDir:officialReportsRoot});
const paperExecutor=pc.config?pcPaperExecutor(pc.config):null;
const learning=withoutRetiredLearningArguments(pc.remainingArgs);
const config=await dailyRecordToolConfiguration(learning.remainingArgs);
// Native launches always run the owner workflow with its store inside the owner
// home, wherever the flags appear. The store is created only after the native
// owner launch is verified, so a refused launch leaves no directory behind.
const nativeLaunch=config.remainingArgs.includes('--dailyrecord-native');
const ownerRoot=nativeLaunch?{requiredRoot:nativeOwnerArtifactsRoot,nativeDefaultStore:nativeOwnerArtifactsRoot+'dailyrecord-workflow'}:{};
const workflowArgs=dailyRecordWorkflowConfiguration(config.remainingArgs,ownerRoot);
const native=await nativeDailyRecordConfiguration(workflowArgs.remainingArgs,config.saveAdmission,config.saveStoreDir,config.saveRequestPath,memoryBindings.reader);
const workflow=workflowArgs.storeDir?createDailyRecordWorkflow({storeDir:workflowArgs.storeDir,...(nativeLaunch?{requiredRoot:nativeOwnerArtifactsRoot}:{})}):null;
const server=new McpServer({name:'academy-dailyrecord-review',version:'0.1.0'});
installToolAudit(server,{endpoint:'dailyrecord',...(nativeLaunch?{root:nativeOwnerArtifactsRoot+'mcp-tool-audit'}:{})});
registerRuntimeIdentity(server,runtimeIdentity.identity);
registerCismWeeklyTools(server,native?.backend??null,nativeOwnerArtifactsRoot+'cism-weekly');
registerTextbookSourceTools(server,native?.backend??null,nativeOwnerArtifactsRoot+'textbook-samples',textbookReader.reader);
registerDailyRecordTools(server,new DailyRecordService(native?.backend??null,config.parentSafeguard,Date.now,null,native?.backend??null,workflow));
registerOfficialReportPdfTool(server,new OfficialReportPdfService(native?.backend??null,pc.executor,officialReportsRoot));
const paperPrint=new PaperPrintPdfService(native?.backend??null,paperExecutor,join(nativeOwnerArtifactsRoot,'print-pdfs'));
registerPaperPrintPdfTool(server,paperPrint);
registerLmsScheduleTool(server,new LmsScheduleService(native?.backend??null));
// An LMS write answer the clinic save cannot parse is kept here as private evidence (specs/academy-mcp/gated-writes.md, "Clinic and wrong-note papers").
registerClinicPaperTools(server,new ClinicPaperService(native?.backend??null,gatedLedger.path,{printer:paperPrint,evidenceRoot:nativeOwnerArtifactsRoot+'clinic-write/http-ack'}));
registerExamPaperTools(server,new ExamPaperService(native?.backend??null,gatedLedger.path));
if(native)process.once('exit',native.close);
installStdioDiagnostics(server.server);
await server.connect(createAcademyStdioTransport());
