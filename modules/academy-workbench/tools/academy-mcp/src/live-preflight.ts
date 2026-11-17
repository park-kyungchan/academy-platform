// Live-backend injection preflight (D2 §4).
// Pure presence-flag assessment. Takes booleans only: never secret values,
// never student identities, never network access. No date window is part of it.

export interface InjectionPresence {
  hasFlockLock: boolean;
  hasRunner: boolean;
  hasDocker: boolean;
  hasImage: boolean;
  hasPinnedArtifacts: boolean;
  hasEnvGetterKey: boolean;
}

export interface InjectionReadiness {
  ready: boolean;
  missing: string[];
}

const LINKS: Array<[keyof InjectionPresence, string]> = [
  ['hasFlockLock', 'account_lock_unavailable'],
  ['hasRunner', 'runner_unavailable'],
  ['hasDocker', 'container_runtime_unavailable'],
  ['hasImage', 'pinned_image_absent'],
  ['hasPinnedArtifacts', 'pinned_artifacts_absent'],
  ['hasEnvGetterKey', 'credential_getter_key_absent'],
];

export function assessInjectionReadiness(
  presence: InjectionPresence,
): InjectionReadiness {
  const missing = LINKS.filter(([key]) => !presence[key]).map(([, code]) => code);
  return { ready: missing.length === 0, missing };
}
