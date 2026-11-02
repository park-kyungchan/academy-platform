# S09 original-reference executor margin v2

This migration-only executor fixes independent review F1: the v1 full 190-case original-source run could hit its own 40-second CPU ceiling while the selected original behavior includes two actual 20-second abort cases. The frozen v1 reference, original TypeScript, fixtures, expected outputs, judge, normalization and effect observers remain immutable at `65e690b76884c8e9c01acb55b89086c8a218c9e7`.

The executor is copied from v1 with only three budget defaults changed: CPU soft/hard 40 to 90 seconds, callable wall default 45 to 90 seconds, and CLI wall default 48 to 90 seconds. Explicit smaller caller deadlines still apply. Output remains 16 MiB, file size 64 MiB, core dumps disabled; streamed capture, group kill, process-local subreaper, descendant reaping and persisted raw channels are unchanged. This is an outer migration-test budget, not a changed original transport deadline or production permission.

Use explicit original source/reference roots and a NEW owned output directory. No source configuration or real account endpoint is started. For this task:

```sh
python3 -B migration/source-adapter-reference-v2/run.py \
 --source-root /home/palantirkc/work/ws-replan/wt/education-platform \
 --reference-root /home/palantirkc/work/ws-replan/wt/education-platform/migration/source-adapter-reference-v1 \
 --output /home/palantirkc/work/ws-replan/preparation/slices/S09/executor-margin-v2/native/NEW
python3 -B /home/palantirkc/work/ws-replan/wt/education-platform/migration/source-adapter-reference-v1/judge.py \
 --reference-root /home/palantirkc/work/ws-replan/wt/education-platform/migration/source-adapter-reference-v1 \
 --result /home/palantirkc/work/ws-replan/preparation/slices/S09/executor-margin-v2/native/NEW
```

Independent reviewers substitute NEW output paths under their own assigned review root. Do not freeze/regenerate or copy expected outputs to a candidate. Full S09, the genuine wire join, account namespace, root ownership, real content binding, Rust mixed execution and P8/P9 remain separate obligations. Root verifies actual fresh full output and bounded-child cleanup, then the same independent reference reviewer resolves F1. Source/reference qualification is not Rust or live acceptance.
