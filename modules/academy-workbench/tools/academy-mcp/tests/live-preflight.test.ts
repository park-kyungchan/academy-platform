import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { assessInjectionReadiness, type InjectionPresence } from '../src/live-preflight.ts';

const ALL_ON: InjectionPresence = {
  hasFlockLock: true,
  hasRunner: true,
  hasDocker: true,
  hasImage: true,
  hasPinnedArtifacts: true,
  hasEnvGetterKey: true,
};

const ALL_OFF: InjectionPresence = {
  hasFlockLock: false,
  hasRunner: false,
  hasDocker: false,
  hasImage: false,
  hasPinnedArtifacts: false,
  hasEnvGetterKey: false,
};

describe('live injection preflight (D2, synthetic flags only)', () => {
  test('full chain present reports ready with no blockers', () => {
    expect(assessInjectionReadiness(ALL_ON)).toEqual({ ready: true, missing: [] });
  });
  test('broken chain reports every missing link by name, never a secret value', () => {
    const result = assessInjectionReadiness(ALL_OFF);
    expect(result.ready).toBe(false);
    expect(result.missing).toHaveLength(6);
    for (const code of result.missing) {
      expect(code).toMatch(/^[a-z0-9_]+$/);
    }
  });
  test('mapping schema ships exactly the one live-proven binding', () => {
    const schema = JSON.parse(
      readFileSync(join(import.meta.dir, '..', 'docs', 'saved-to-native-id-mapping.schema.json'), 'utf8'),
    );
    expect(schema.status).toBe('partially_proven');
    expect(schema.bindings).toHaveLength(2);
    expect(schema.bindings[0]).toMatchObject({
      savedStudentId: 'stu_c7c07d590b374817a8169633d2b625e7',
      nativeStudentId: '1293067',
      evidence: {source: 'live DT inventory observation', observedAt: '2026-10-26T12:40:00Z'},
    });
  });
});
