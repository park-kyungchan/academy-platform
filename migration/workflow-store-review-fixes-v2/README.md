# New original-backed review regressions

Read `docs/s10/REVIEW-FIXES-v2.md` and the task's ROOT-DISPOSITIONS/allocation.
Compiler/check HOLD applies until Root grants the sole slot. No live source,
native account, service, private fixture acquisition or remote Git is involved.

After grant, substitute NEW absolute private evidence roots and own binary:

```text
python3 -B migration/workflow-store-review-fixes-v2/verify.py freeze --root OWN_NATIVE/original-01
python3 -B migration/workflow-store-review-fixes-v2/verify.py compare --root OWN_NATIVE/regression-01 --binary OWN_BINARY
```

Run through the unchanged `migration/workflow-store-rust-v1/run_check.py` to
retain raw outer channels/status/duration/cleanup. Set
`PYTHONDONTWRITEBYTECODE=1`. The Python observer disables bytecode before imports.
Freeze runs only actual original TS with deny.ts preloaded; it refuses an
existing expectation. Comparison runs only actual Rust operations with explicit
fictional typed ports. No Rust result, old oracle or TS result is a runtime
fallback. CASES is shared input data; all expected results originate from the
original class before Rust comparison.

The regression output compares complete result values and exact review digests,
including row suggestions/meaning/nulls and callback arguments. Only optional
entries at the JSON RPC tracing boundary use explicit null for JS undefined;
the direct Rust test requires the original optional value at learning_suggest.
No comparison normalization is added or changed. Held reviews must preserve all
journal bytes and invoke no authentication, approval, save, PDF/print or send.
