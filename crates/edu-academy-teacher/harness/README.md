# Measurement harness (slice 1 repair)
`python3 run.py Q01|Q02 <model> --mode isolated|installed` runs exactly one session.
Both modes use a per-run strict MCP config, an empty working directory, stream-json with verbose output, and no session persistence.
`isolated` adds `--bare --setting-sources "" --tools "" --disable-slash-commands`: no user settings, hooks, plugins, auto-memory, CLAUDE.md discovery, built-in tools or skills/commands.
`installed` loads the installed user settings and built-in tools as packet B did; no configuration is edited.
Each temporary run directory holds mcp.json, run-log.jsonl, claude.jsonl, claude.stderr, answer.txt and measurement.json.
Tool names/inputs, last received tool-result text, answer, usage and cost come only from the stream; argument bytes use compact UTF-8 JSON.
`verbatim` compares whitespace-normalised text; `extra_chars` counts normalised answer characters outside its first complete display occurrence (all answer characters if none). Missing evidence is `unknown`.
Answer date/forbidden-date/order/line checks use checks.json; failed measurements never justify changing the server, list or displays.
Offline tests: `python3 -B -m unittest discover -s crates/edu-academy-teacher/harness`; overrides: EDU_ACADEMY_TEACHER_BINARY and CLAUDE_BIN.
