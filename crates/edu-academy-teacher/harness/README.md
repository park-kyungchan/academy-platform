# Measurement harness (slice 1 repair)
`python3 run.py Q01|Q02 <model> --mode isolated|installed` runs exactly one session.
Both modes use a per-run strict MCP config, an empty working directory, stream-json with verbose output, and no session persistence.
`isolated` uses `--setting-sources "" --tools "" --disable-slash-commands --system-prompt "You are an assistant of a teacher. Use the tools you are given."`; existing CLI login is retained without reading or passing credentials.
`installed` loads the installed user settings and built-in tools as packet B did; no configuration is edited.
Each temporary run directory holds mcp.json, run-log.jsonl, claude.jsonl, claude.stderr, answer.txt and measurement.json.
Tool names/inputs, last received tool-result text, answer, usage and cost come only from the stream; argument bytes use compact UTF-8 JSON.
`verbatim` compares whitespace-normalised text; `extra_chars` counts normalised answer characters outside its first complete display occurrence (all answer characters if none). Missing evidence is `unknown`.
Answer date/forbidden-date/order/line checks use checks.json; failed measurements never justify changing the server, list or displays.
Offline tests: `python3 -B -m unittest discover -s crates/edu-academy-teacher/harness`; overrides: EDU_ACADEMY_TEACHER_BINARY and CLAUDE_BIN.
The exact command and fixed system prompt are saved in measurement.json; no authentication environment is changed.
Isolation checks init fields tools, slash_commands, terminal_slash_commands, skills, plugins, agents and mcp_servers when present; missing tools/init/first-request usage fails closed.
Only the 14 configured Academy tools and default CLI agents claude, Explore, general-purpose, Plan, statusline-setup are allowed. The agents were observed in the prior all-sources-disabled init (/tmp/p8-isolated-Q01-haiku-gtye4xxn/run/claude.jsonl).
Any plugin (including builtin), skill, slash command or hook stream event fails isolation; all existing inspected init fields and values are recorded. Native memory_paths metadata is also recorded, but an auto-memory destination does not prove a file load. The first assistant message's input+cache-read+cache-creation must be under 4,000 tokens; exit 3 signals a failed token bound and requires stopping the run series.
