# S11 strict reference qualifier v2

Read [CONTRACT.md](../../migration/mcp-runtime-qualifier-v2/CONTRACT.md),
[POLICY.json](../../migration/mcp-runtime-qualifier-v2/POLICY.json) and
[BASELINE-MANIFEST.json](../../migration/mcp-runtime-qualifier-v2/BASELINE-MANIFEST.json).
Original v1 commit `807db88af175b4dc60b843584e2cc62101e1ac31` and every v1
observer/frozen/expected/manifest/graph/dependency byte remain preserved.

This corrective comparator addresses Wonder's HIGH F1 global normalization and
HIGH F2 JSON-only comparison findings. It compares original raw data and
provenance, preserves literal semantic values/text, rejects duplicate-key JSON,
requires top and inner capture files, checks channel hashes, exact process exits,
source-before/after and original-derived request/response/receipt roles. Only
enumerated, bounded observer elapsedSeconds may vary. Payload elapsedMs, source
freshness, semantic UUIDs, process identity, locators and digests remain literal.

```sh
python3 -B migration/mcp-runtime-qualifier-v2/qualify.py --verify-baseline
python3 -B migration/mcp-runtime-qualifier-v2/test_qualify.py
python3 -B migration/mcp-runtime-qualifier-v2/qualify.py --mode protocol --candidate /absolute/complete-capture
```

The candidate directory format is the exact per-mode candidateFiles set in the
manifest. observations.json alone is refused. No command launches a candidate,
captures native processes, generates expectations or accesses embedded native
locators. Evidence must already exist. Synthetic transformed originals used by
tests are labeled as such, not fresh candidate execution.

All ten modes remain **LIMITED** even when recorded-boundary comparison matches:
exit 2 is not success/acceptance. Exit 1 means refused/unqualified; exit 0 from
--verify-baseline means integrity only. New semantic runtime UUID/clock values
and relocated logical roots are explicitly unsupported and refused. Source
observedAt is freshness-sensitive; its future bounded provenance proposal is
recorded but not implemented from insufficient v1 evidence.

The contract discloses v1 misassociated per-process direct copies, JS tap/no-event
limits, missing graph receipts/inner EXIT, and four observer-only outcomes. It
does not upgrade fictional topology, dynamic bundle/jsdom closure, active
confinement, early failed-attempt stderr or no-fetch limits into proof.

No graph admission/native equivalence, Rust facade, production effect or P9
authority follows. Actual graph/native owners remain external prerequisites.
This immutable qualifier must receive new independent Wonder review and Root
acceptance before facade work; the implementation author cannot self-accept it.
