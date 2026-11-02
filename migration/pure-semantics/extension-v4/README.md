# S08 independent original-source characterization v4

Start with [SUMMARY.json](SUMMARY.json), [CONTRACT.md](CONTRACT.md), [HANDOFF.md](HANDOFF.md), [source-dispositions.json](source-dispositions.json) and [PROVENANCE.json](PROVENANCE.json). Candidate0551ea4 was read statically only. This is a migration-only oracle foundation, not a runtime harness. Independent Wonder review is required before Root oracle acceptance; it claims no S08/S09/S10 acceptance.

595 new source cases,32 actual-child negative controls,8 strict protocol/scope controls,52 meaningful source assertions. Old202 unchanged. Both expected files were generated exclusively by unchanged original functions with deterministic ports before invocation, never candidate output. Earlier failures and first successful freezes are retained.

Reproduce against existing dependencies, into **new** directories (never use --freeze to rerun):

```bash
node migration/pure-semantics/extension-v4/run.mjs \
  --deps-root /home/palantirkc/projects/education-platform/modules/academy-workbench/tools/academy-mcp/node_modules \
  --results migration/pure-semantics/extension-v4/raw/review-main-01
node migration/pure-semantics/extension-v4/run-supplement.mjs \
  --results migration/pure-semantics/extension-v4/raw/review-supplement-01
node migration/pure-semantics/extension-v4/check-source-meaning.mjs \
  migration/pure-semantics/extension-v4/raw/review-meaning-01.json
```

Bun1.4.3 and Nodev24.22.1 are required; dependency/source bytes are pinned. Each source child is capped at5seconds and8MiB capture. Raw actual argv/status/stdout/stderr are preserved. Source snapshots, synthetic local request files and empty sanitized homes remain under ignored `raw/*/workspace/`; their original source hashes are pinned. Ignore rules affect these new disposable test workspaces only, not old retained artifacts. The real installed packages are read-only. No install or production/native/global path is needed.

Authoring scripts are retained for provenance, not rerun against immutable destinations. `create-supplement.mjs` is the preserved failed v1 collector; `create-supplement-v2.mjs` corrects package-root discovery. Their source inputs and first resulting artifacts were not overwritten. Full expected files, cases, judge and all baseline programs are listed in `freeze.json`.
