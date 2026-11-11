# Measurement harness (slice 1 repair)
`python3 run.py Q01|Q02 <model> --mode isolated|installed` runs exactly one session.
Both modes use a per-run strict MCP config, an empty working directory, stream-json with verbose output and --include-hook-events, and no session persistence.
`isolated` uses `--setting-sources "" --tools "" --disable-slash-commands --system-prompt "You are an assistant of a teacher. Use the tools you are given."`; existing CLI login is retained without reading or passing credentials.
`isolated` measures the server's own texts alone, with the fixed neutral system prompt and user setting sources disabled.
`installed` measures the same with the user's instructions, skills and hooks loaded, without file or shell tools.
Each temporary run directory holds mcp.json, run-log.jsonl, claude.jsonl, claude.stderr, answer.txt and measurement.json.
Tool names/inputs, the last Academy result matched by tool_use_id, answer, usage and cost come only from the stream; argument bytes use compact UTF-8 JSON. Other tools never supply the display reference.
`received_form` is envelope when received JSON has a string display (that field is the reference), otherwise display_text; no Academy result is unknown. `verbatim` compares whitespace-normalised reference text; `extra_chars` counts normalised answer characters outside its first complete display occurrence (all answer characters if none). Missing evidence or an empty/whitespace-only reference is `unknown`.
Answer date/forbidden-date/order/line checks use checks.json; failed measurements never justify changing the server, list or displays.
Offline tests: `python3 -B -m unittest discover -s crates/edu-academy-teacher/harness`; overrides: EDU_ACADEMY_TEACHER_BINARY and CLAUDE_BIN.
The exact command and fixed system prompt are saved in measurement.json; no authentication environment is changed.
Isolation checks init fields tools, slash_commands, terminal_slash_commands, skills, plugins, agents and mcp_servers when present; missing tools/init/first-request usage fails closed.
Only the 14 configured Academy tools and CLI agents claude, Explore, general-purpose, Plan, statusline-setup, web-fetch are allowed. Every listed plugin must have path=builtin.
Non-builtin plugins, skills, slash commands or hook stream events fail isolation; all existing inspected init fields and values are recorded. Native memory_paths metadata is also recorded, but an auto-memory destination does not prove a file load. The first assistant message's input+cache-read+cache-creation must be under 6,000 tokens; exit 3 signals a failed token bound and requires stopping the run series.
Installed adds exactly `--tools Skill` alongside the per-run Academy allow list; it retains user settings, instructions, skills and hooks. No configuration file is edited.
Both modes request --include-hook-events; isolation records hook_condition as pass/FAIL only with that flag. Older streams give hook_condition=unknown and pass (hooks unknown) if the other isolation checks pass.
Offline re-scoring: python3 -B crates/edu-academy-teacher/harness/rescore.py <run-dir> [...]. It uses the same score_stream as live runs, reads evidence without modifying it, and emits only mode/request/model/received_form/verbatim/extra_chars/isolation. No tool arguments, file contents, answers or reference text leave the saved directories.
Installed rows report isolation as `not_applicable`; the retained hook evidence still records hook visibility. Strict isolation criteria apply to isolated rows.
