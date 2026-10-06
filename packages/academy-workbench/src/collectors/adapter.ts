import type { CanonicalSnapshot } from '../domain/canonical.ts';
/** Only a boundary declaration. No browser, network, cookie or credential access. */
export interface SourceCollectorAdapter {
  readonly mode: 'synthetic' | 'allowed-dom-read-batch';
  readonly capabilities: { readonly readOnly: true; readonly directHttpVerified: false };
  collect(): Promise<CanonicalSnapshot>;
}
export class SyntheticCollector implements SourceCollectorAdapter {
  readonly mode = 'synthetic';
  readonly capabilities = { readOnly: true, directHttpVerified: false } as const;
  constructor(private readonly fixture: CanonicalSnapshot) {}
  async collect(): Promise<CanonicalSnapshot> { return structuredClone(this.fixture); }
}
