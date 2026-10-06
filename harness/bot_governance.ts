import { botDevelopmentContract, parseBotRequest, syntheticBotRequest, type BotDevelopmentRequest } from '../docs/bot-development-contract';
import { botDevelopmentRunbook } from '../docs/bot-development-runbook';

export interface BotDevelopmentDecision {
  schemaVersion: 1;
  kind: 'advisory_bot_development_decision';
  permissionGranted: false;
  nativeEvidenceVerified: false;
  executesEffects: false;
  disposition: 'stop' | 'human_handoff' | 'reconcile_readback' | 'collect_native_evidence' | 'native_guarded_step';
  nextStep: string;
  consumer: 'browser_exec' | 'computer_use' | 'native_bot_messaging' | 'native_room' | 'native_cron' | 'native_screen_cli' | 'native_engineering';
  reasons: string[];
  requiredNativeEvidence: string[];
  inputTrust: 'untrusted_metadata_claims_only';
  completionBoundary: 'CLI_computation_not_native_execution_or_product_acceptance';
}

/** Deterministic advisory evaluation; no native state is read, changed, mirrored or authenticated here. */
export function decideBotDevelopment(value: unknown, nowMs = Date.now()): BotDevelopmentDecision {
  const r = parseBotRequest(value);
  if (!Number.isFinite(nowMs)) throw new Error('decision:valid_clock_required');
  const { assignment: a, caller: c, target: t, screen: s, messaging: m, remote, execution: e } = r;
  const reasons: string[] = [];
  const add = (condition: boolean, code: string) => { if (condition) reasons.push(code); };
  const gui = ['browser_dom', 'native_chrome', 'screen_start'].includes(r.action);
  const directMessage = r.action === 'bot_message';
  const message = directMessage || r.action === 'room_turn';
  // The room entry is a native RPC, not an injected model-facing tool. These names are
  // assignment scope declarations only; no tool/RPC/CLI is dispatched by this module.
  const toolForAction: Record<BotDevelopmentRequest['action'], string> = {
    engineering: 'terminal', browser_dom: 'browser_exec', native_chrome: 'computer_use', screen_start: 'terminal', bot_message: 'message_agent', room_turn: 'groups.send', routine: 'cronjob_manage',
  };
  const observed = Date.parse(r.observation.observedAtUtc);
  const until = Date.parse(r.observation.validUntilUtc);
  add(observed > nowMs || until <= nowMs || until <= observed, 'observation_not_current');
  add(!a.purpose.trim() || !a.completion.trim() || !a.inputs.length || !a.ownedPaths.length || !a.outputs.length || !a.verification.length || !a.tools.length, 'complete_work_required');
  add(!a.tools.includes(toolForAction[r.action]), 'tool_outside_assignment');
  add(a.ownerProfile !== botDevelopmentContract.profile || c.profile !== a.ownerProfile, 'caller_profile_mismatch');
  add(!a.freshBudget, 'fresh_assignment_required');
  add(e.turnsUsed >= a.maxTurns || e.elapsedSeconds >= a.maxSeconds, 'budget_exhausted');
  add(e.activeAssignments >= a.maxConcurrency, 'concurrency_exhausted');
  add(e.retryCount > a.maxRetries, 'retry_exhausted');
  add(e.model !== a.model || e.provider !== a.provider || e.reasoning !== a.reasoning, 'runtime_selection_mismatch');
  // Effort is selected for this Work, not imposed by role. Strict ingress admits
  // supported efforts; the preceding runtime check requires the exact selection.
  add(a.provider !== 'openai-codex' || (a.model === 'gpt-6-luna' && (a.role !== 'restricted_worker' || a.judgment !== 'negligible')), 'role_model_mismatch');
  // Preserve the observed Lead's exact model ID; this neither selects a model
  // nor renews the base-Sol worker/reviewer assignment controls.
  add(a.model === botDevelopmentContract.modelSelection.observedLeadModel && a.role !== 'trusted_lead', 'role_model_mismatch');
  add(a.role !== 'trusted_lead' && c.restrictions !== 'native_enforced', 'native_restrictions_required');
  add(a.role === 'restricted_reviewer' && (r.action !== 'engineering' || t.effect !== 'repo_read' || a.effects.some(x => x !== 'repo_read')), 'reviewer_read_only');
  add(e.approvalMode === 'off' || e.headlessPolicy !== 'deny' || !e.redaction, 'native_safety_required');
  add(e.nativeDecision === 'denied', 'native_denial');
  add(e.nativeDecision === 'unknown', 'native_decision_unknown');
  add(t.effect === 'unknown' || a.effects.includes('unknown'), 'unknown_effect');
  add(!a.effects.includes(t.effect), 'effect_outside_assignment');
  add(t.identity !== 'owned_exact', 'exact_target_identity_required');
  add(remote.requestOrigin === 'unknown' || remote.route !== 'qualified_backend', 'remote_route_required');
  add(remote.pathOwner !== 'linux_backend', 'backend_path_required');
  add(remote.artifactOwner !== 'linux_backend', 'artifact_owner_required');
  add(e.retryCount > 0 && e.readback !== 'same_target', 'retry_readback_required');
  add(e.previousEffect === 'uncertain', 'uncertain_effect');

  if (directMessage) {
    add(c.sessionKind !== 'canonical_bot_chat', 'canonical_bot_chat_required');
    add(c.canonicalTitle !== 'Bot Chat' || !c.managedInstall || !c.protocolEnabled || !m.toolExposed || !m.toolDispatchable, 'native_message_injection_required');
  }
  if (message) {
    add(m.admission !== 'current_assignment', 'message_admission_required');
    add(m.route !== 'exact', 'exact_peer_route_required');
    add(m.payload !== 'minimized_synthetic', 'message_disclosure_required');
    add(t.effect !== 'external_message', 'action_effect_mismatch');
  }
  if (r.action === 'room_turn') {
    add(m.roomHeld, 'room_held');
    add(m.roomEpochAdmitted !== m.roomEpochCurrent, 'room_authority_drift');
    add(m.roomPolicy !== 'target_native_current', 'native_room_policy_required');
  }
  if (r.action === 'routine') {
    add(m.routineAdmission !== 'bounded_native', 'routine_admission_required');
    add(t.effect !== 'routine_activation', 'action_effect_mismatch');
  }
  if (r.action === 'engineering') add(!['repo_read', 'repo_write'].includes(t.effect) || t.surface !== 'repository', 'action_effect_mismatch');
  if (gui) {
    add(r.rehearsal.source !== 'existing_date_class', 'existing_rehearsal_source_required');
    add(r.rehearsal.buildTests !== 'passed', 'build_test_before_gui_required');
    add(r.rehearsal.inactivity !== 'native_120s_no_keepalive', 'preserve_native_inactivity_no_keepalive');
    add(r.rehearsal.inheritedHumanOrWindowsAcceptance, 'historical_acceptance_not_inherited');
    add(c.binding !== 'native_tool_context', 'actual_native_caller_binding_required');
    add(!c.guiToolExposed, 'native_gui_tool_required');
    add(!s.supported || !s.installed, 'screen_unavailable');
    add(s.profile !== c.profile, 'screen_profile_mismatch');
    add(s.placement === 'unknown', 'screen_placement_unqualified');
    add(s.holder === 'human', 'human_has_control');
    add(s.holder === 'unknown', 'lease_unknown');
    add(s.epochAdmitted !== s.epochCurrent, 'lease_epoch_drift');
    add(s.afterHumanHandoff && !s.userContinue, 'user_continue_required');
    add(s.otherOwners !== 'none' || s.batchOwner !== c.sessionId, 'exclusive_gui_owner_required');
    add(a.guiConcurrency !== 1, 'one_owner_gui_batch_required');
    add(t.challenge !== 'none', 'human_challenge');
    add(t.kind === 'synthetic' && (t.origin !== 'backend_local' || t.privateData), 'synthetic_local_target_required');
    add(t.kind === 'product', 'product_authorization_independent');
    add(t.kind === 'academy_source', 'academy_direct_http_required');
    add(t.effect !== 'synthetic_gui', 'action_effect_mismatch');
    if (r.action !== 'screen_start') {
      add(!r.rehearsal.freshPage, 'fresh_owned_page_required');
      add(!s.running, 'screen_not_running');
      add(r.action === 'browser_dom' && t.surface !== 'browser_dom' || r.action === 'native_chrome' && t.surface !== 'native_chrome', 'surface_consumer_mismatch');
      add(t.browserProvenance !== 'native_managed_local', 'owned_native_browser_required');
    }
    if (r.action === 'native_chrome') {
      add(r.rehearsal.browserCases !== 'passed_current_source', 'browser_cases_before_screen_required');
      add(!r.rehearsal.immediateCapture, 'immediate_same_profile_screen_required');
    }
  }

  const consumers: Record<BotDevelopmentRequest['action'], BotDevelopmentDecision['consumer']> = {
    engineering: 'native_engineering', browser_dom: 'browser_exec', native_chrome: 'computer_use', screen_start: 'native_screen_cli', bot_message: 'native_bot_messaging', room_turn: 'native_room', routine: 'native_cron',
  };
  const requiredNativeEvidence = [
    'current user assignment and complete Work with owner/input/output/verification/recovery',
    'actual model/provider/reasoning, fresh finite turn/time/concurrency/retry budget and completion contract',
    'actual native approval, headless deny and redaction; no serialized claim grants authority',
    'exact target/effect identity and safe same-target readback before any retry',
    'actual Backend route, request origin and artifact/path owner independent of Windows viewer',
  ];
  if (a.role !== 'trusted_lead') requiredNativeEvidence.push('actual native enforced worker/reviewer restrictions; Profile and tool names are not an OS sandbox');
  if (directMessage) requiredNativeEvidence.push('native canonical Bot Chat lineage, managed install, enabled protocol and exposed/dispatchable message_agent');
  if (message) requiredNativeEvidence.push('exact peer/backend/profile and current minimized disclosure admission');
  if (r.action === 'room_turn') requiredNativeEvidence.push('current native room authority/epoch, member held/stop state and target-issued native execution policy including tools/approval/iterations');
  if (r.action === 'routine') requiredNativeEvidence.push('separately admitted native routine envelope, delivery/cost/recovery and actual native scheduling controls');
  if (gui) requiredNativeEvidence.push(
    'actual native tool caller context-local profile/home (not shell/cwd/config metadata)',
    'fresh native supported/installed/running Screen status, exact profile and placement',
    'native current agent/free lease and matching pre/post epoch; no capture/input during human control',
    'coordinated one-owner GUI batch with no other current Screen/browser owner',
    'owned synthetic local page/window identity, no private tabs/data and native managed browser provenance',
    'after any human handoff: actual handback plus explicit user continue, then safe fresh inspection',
    'existing academy_browser_rehearsal and current dateClassBrowserCases source: build/test first, fresh owned page/lease, browser cases, immediate same-profile composited Screen, owned cleanup/readback',
    'native 120-second inactivity unchanged, no keepalive; reacquire a fresh owned page after native cleanup, never revive stale targets',
    'historical 9-case receipt and current 11-case source are not this worker live trial, real DB, human handoff or Windows acceptance',
  );

  let disposition: BotDevelopmentDecision['disposition'] = 'native_guarded_step';
  let nextStep = 'revalidate_native_evidence_then_use_existing_guarded_consumer';
  // Never route around a native denial, even if another independent lane could continue.
  if (reasons.includes('native_denial') || reasons.includes('native_safety_required')) {
    disposition = 'stop'; nextStep = 'preserve_native_refusal_no_equivalent_bypass';
  } else if (reasons.includes('human_has_control') || reasons.includes('human_challenge') || reasons.includes('user_continue_required')) {
    disposition = 'human_handoff'; nextStep = 'end_turn_without_capture_or_input';
  } else if (reasons.includes('uncertain_effect') || reasons.includes('lease_epoch_drift') || reasons.includes('room_authority_drift') || reasons.includes('retry_readback_required')) {
    disposition = 'reconcile_readback'; nextStep = 'discard_unsafe_result_then_reconcile_original_target_without_replay';
  } else if (reasons.includes('academy_direct_http_required')) {
    disposition = 'stop'; nextStep = 'use_qualified_direct_http_contract';
  } else if (reasons.includes('product_authorization_independent')) {
    disposition = 'stop'; nextStep = 'route_to_product_authorization_owner';
  } else if (reasons.some(code => ['unknown_effect', 'effect_outside_assignment', 'action_effect_mismatch', 'reviewer_read_only', 'budget_exhausted', 'retry_exhausted', 'concurrency_exhausted', 'room_held'].includes(code))) {
    disposition = 'stop'; nextStep = 'preserve_scope_or_budget_boundary_without_effects';
  } else if (reasons.length) {
    disposition = 'collect_native_evidence';
    nextStep = reasons.length === 1 && reasons[0] === 'screen_not_running'
      ? 'check_native_screen_start_prerequisites' : 'resolve_reported_gaps_without_effects';
  } else if (e.previousEffect === 'confirmed') {
    disposition = 'stop'; nextStep = 'retain_readback_do_not_repeat';
  }
  return {
    schemaVersion: 1, kind: 'advisory_bot_development_decision', permissionGranted: false, nativeEvidenceVerified: false, executesEffects: false,
    disposition, nextStep, consumer: consumers[r.action], reasons, requiredNativeEvidence,
    inputTrust: 'untrusted_metadata_claims_only', completionBoundary: 'CLI_computation_not_native_execution_or_product_acceptance',
  };
}

export function runBotGovernanceCli(args: string[], input = '', nowMs = Date.now()): { exitCode: number; stdout: string } {
  const emit = (data: unknown, exitCode = 0) => ({ exitCode, stdout: JSON.stringify(data, null, 2) + '\n' });
  try {
    if (args.length === 1) {
      switch (args[0]) {
        case '--help': return emit({ command: 'bun run harness/bot_governance.ts', operations: ['--help', 'contract', 'runbook', 'example', 'decide --stdin'], maxInputBytes: 65536, secretsAllowed: false, effectMode: 'advisory_only_no_execution', exitCodes: { 0: 'decision_or_view_computed_not_permission', 2: 'invalid_input_or_arguments' } });
        case 'contract': return emit(botDevelopmentContract);
        case 'runbook': return emit(botDevelopmentRunbook);
        case 'example': {
          const r = structuredClone(syntheticBotRequest);
          const iso = (ms: number) => new Date(ms).toISOString().replace(/\.\d{3}Z$/, 'Z');
          r.observation = { observedAtUtc: iso(nowMs), validUntilUtc: iso(nowMs + 60000) };
          return emit(r);
        }
      }
    }
    if (args.length !== 2 || args[0] !== 'decide' || args[1] !== '--stdin') throw new Error('cli:invalid_arguments');
    if (Buffer.byteLength(input, 'utf8') > 65536) throw new Error('cli:input_too_large');
    let raw: unknown;
    try { raw = JSON.parse(input); } catch { throw new Error('cli:invalid_json'); }
    return emit(decideBotDevelopment(raw, nowMs));
  } catch (error) {
    // Only validator-generated paths/codes are emitted; never provider errors, JSON excerpts or user values.
    const safe = error instanceof Error && /^(?:request(?:\.[A-Za-z0-9]+)*|cli|decision):[a-z_]+$/.test(error.message)
      ? error.message : 'cli:invalid_input';
    return emit({ schemaVersion: 1, kind: 'advisory_bot_input_rejection', permissionGranted: false, executesEffects: false, error: safe }, 2);
  }
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let input = '';
  if (args.length === 2 && args[0] === 'decide' && args[1] === '--stdin') {
    // Bounded streaming ingress. Do not load an arbitrarily large/private file before validating.
    const chunks: Buffer[] = [];
    let bytes = 0;
    for await (const chunk of process.stdin) {
      const buffer = Buffer.from(chunk);
      bytes += buffer.byteLength;
      if (bytes > 65536) {
        process.stdout.write(JSON.stringify({ schemaVersion: 1, kind: 'advisory_bot_input_rejection', permissionGranted: false, executesEffects: false, error: 'cli:input_too_large' }) + '\n');
        process.exit(2);
      }
      chunks.push(buffer);
    }
    input = Buffer.concat(chunks).toString('utf8');
  }
  const result = runBotGovernanceCli(args, input);
  process.stdout.write(result.stdout);
  process.exitCode = result.exitCode;
}
