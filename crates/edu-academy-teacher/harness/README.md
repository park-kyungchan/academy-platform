# Measurement harness (slice 1, packet B)
`run.py Q01|Q02 <model>` runs one measured Claude run against the built binary only.
It writes a per-run directory under a `mktemp -d` root holding `mcp.json`, `run-log.jsonl`, `answer.txt` and `claude.json`.
The MCP configuration's only server is `edu-academy-teacher --scenario <abs> --today 2026-11-11 --run-log <run>/run-log.jsonl`.
The request text goes to `claude --print` on stdin from a neutral empty working directory; no user configuration is edited.
It prints one row: model, calls/target, codes, summed `args_bytes`, display-verbatim, `checks.json` results, wall time, tokens and cost.
`checks.json` is the hand-encoded assertion set for Q01/Q02; the request TSV is not machine-parsed.
A failed check is a measured result to report, not a reason to change the server, tool list or display in this packet.
Binary override: `EDU_ACADEMY_TEACHER_BINARY`; Claude override for smoke tests: `CLAUDE_BIN`.
Example: `python3 run.py Q01 haiku`
