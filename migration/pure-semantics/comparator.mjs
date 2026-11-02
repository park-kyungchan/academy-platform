import {isDeepStrictEqual} from 'node:util';

// Exact child evidence is input. Never infer status from a plausible JSON body.
export function compare(expected, actual) {
  if (actual.status !== 0 || actual.signal !== null || actual.spawnError !== null) return {accepted: false, reason: 'child_status'};
  if (actual.stderr !== '') return {accepted: false, reason: 'stderr_contamination'};
  let value;
  try { value = JSON.parse(actual.stdout); } catch { return {accepted: false, reason: 'stdout_not_json'}; }
  if (actual.stdout !== JSON.stringify(value) + '\n') return {accepted: false, reason: 'stdout_framing'};
  if (value.effects?.deniedTransportAttempts !== 0 || value.effects?.forbiddenFakeDispatches !== 0) return {accepted: false, reason: 'forbidden_effect'};
  return isDeepStrictEqual(expected, value) ? {accepted: true, reason: 'exact_values'} : {accepted: false, reason: 'value_shape_order_or_bytes'};
}
