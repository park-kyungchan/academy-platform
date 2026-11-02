import {isDeepStrictEqual} from 'node:util';

// The caller supplies the independently frozen oracle; no candidate fallback.
export function compare(expected, actual) {
  if (actual.status !== 0 || actual.signal !== null || actual.spawnError !== null)
    return {accepted: false, reason: 'child_status'};
  if (actual.stderr !== '') return {accepted: false, reason: 'stderr_contamination'};
  let parsed;
  try { parsed = JSON.parse(actual.stdout); }
  catch { return {accepted: false, reason: 'stdout_not_json'}; }
  if (actual.stdout !== JSON.stringify(parsed) + '\n')
    return {accepted: false, reason: 'stdout_framing'};
  return isDeepStrictEqual(expected, parsed)
    ? {accepted: true, reason: 'exact_values'}
    : {accepted: false, reason: 'value_shape_order_or_effect'};
}
