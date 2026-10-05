#!/usr/bin/env bun
/** Offline catalog and scoped native-read preparation; no credentials or HTTP execution. */
import { endpointOutputMetrics, queryEndpoints, type EndpointQuery } from '../src/lms/endpointCatalog';
import {prepareAcademyReadFile,auditAcademyReadFile} from './academy_read_prepare';

export function endpointCli(args: string[]): unknown {
  if(args.includes('--audit-read')||args.includes('--output-audit')) {
    if(args.length!==4||args[0]!=='--audit-read'||args[2]!=='--output-audit'||!args[1]||!args[3])throw new Error('invalid_read_audit_flags');
    return auditAcademyReadFile(args[1],args[3]);
  }
  if(args.includes('--prepare-read')||args.includes('--output-script')) {
    if(args.length!==4||args[0]!=='--prepare-read'||args[2]!=='--output-script'||!args[1]||!args[3])throw new Error('invalid_read_preparation_flags');
    return prepareAcademyReadFile(args[1],args[3]);
  }
  if (args.length===1 && args[0]==='--help') return {
    command:'bun run endpoint:query',nativeAuthentication:'browser_vault_required',
    catalog:'--id <exact-id> | --operation <literal-selector> | --projection coverage',
    readPreparation:'--prepare-read <private-request.json> --output-script <private-script.js>',
    sourceAudit:'--audit-read <private-result.json> --output-audit <private-audit.json>',
    preparedFamilies:['daily_zero','day_record','item_answer','item_metadata','student_course','textbook'],
    blockedPreparation:{},
    policy:'Review the source-specific contract; output scripts perform only the admitted existing-result reads in an authenticated native browser.',
    limits:'Unknown/effectful operations stay blocked. Source declarations are not full backend or SQL access.',
  };
  const query: EndpointQuery = {};
  const seen = new Set<string>();
  let metrics = false;
  const valued = ['--id', '--operation', '--family', '--origin', '--effect', '--projection', '--limit'];
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]!;
    if (seen.has(arg)) throw new Error('duplicate_flag');
    seen.add(arg);
    if (arg === '--callable-only') { query.callableOnly = true; continue; }
    if (arg === '--metrics') { metrics = true; continue; }
    if (!valued.includes(arg)) throw new Error('unsupported_flag');
    const value = args[++i];
    if (!value || value.startsWith('--')) throw new Error('missing_flag_value');
    if (arg === '--id') query.id = value;
    if (arg === '--operation') query.operation = value;
    if (arg === '--family') query.family = value;
    if (arg === '--origin') query.origin = value as EndpointQuery['origin'];
    if (arg === '--effect') query.effect = value as EndpointQuery['effect'];
    if (arg === '--projection') query.projection = value as EndpointQuery['projection'];
    if (arg === '--limit') query.limit = /^\d+$/.test(value) ? Number(value) : NaN;
  }
  // Validate even when metrics selects its own projections.
  const result = queryEndpoints(query);
  return metrics ? { result, metrics: endpointOutputMetrics(query) } : result;
}

if (import.meta.main) {
  try { process.stdout.write(JSON.stringify(await endpointCli(process.argv.slice(2))) + '\n'); }
  catch (error) {
    const code = error instanceof Error && /^[a-z_]+$/.test(error.message) ? error.message : 'endpoint_catalog_failed';
    process.stderr.write(JSON.stringify({ ok: false, code }) + '\n');
    process.exitCode = 1;
  }
}
