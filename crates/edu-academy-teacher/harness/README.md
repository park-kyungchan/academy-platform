# Measurement harness (slice 1 repair)
`python3 run.py Q01|Q02 <model> --mode isolated|installed` runs exactly one session.
Both modes use a per-run strict MCP config, an empty working directory, stream-json with verbose output, and no session persistence.
`isolated` uses `--setting-sources "" --tools "" --disable-slash-commands --system-prompt "You are an assistant of a teacher. Use the tools you are given."`; existing CLI login is retained without reading or passing credentials.
`isolated` measures the server's own texts alone, with the fixed neutral system prompt and user setting sources disabled.
`installed` measures the same with the user's instructions, skills and hooks loaded, without file or shell tools.
Each temporary run directory holds mcp.json, run-log.jsonl, claude.jsonl, claude.stderr, answer.txt and measurement.json.
Tool names/inputs, last received tool-result text, answer, usage and cost come only from the stream; argument bytes use compact UTF-8 JSON.
`verbatim` compares whitespace-normalised text; `extra_chars` counts normalised answer characters outside its first complete display occurrence (all answer characters if none). Missing evidence is `unknown`.
Answer date/forbidden-date/order/line checks use checks.json; failed measurements never justify changing the server, list or displays.
Offline tests: `python3 -B -m unittest discover -s crates/edu-academy-teacher/harness`; overrides: EDU_ACADEMY_TEACHER_BINARY and CLAUDE_BIN.
The exact command and fixed system prompt are saved in measurement.json; no authentication environment is changed.
Isolation checks init fields tools, slash_commands, terminal_slash_commands, skills, plugins, agents and mcp_servers when present; missing tools/init/first-request usage fails closed.
Only the 14 configured Academy tools and CLI agents claude, Explore, general-purpose, Plan, statusline-setup, web-fetch are allowed. Every listed plugin must have path=builtin.
Non-builtin plugins, skills, slash commands or hook stream events fail isolation; all existing inspected init fields and values are recorded. Native memory_paths metadata is also recorded, but an auto-memory destination does not prove a file load. The first assistant message's input+cache-read+cache-creation must be under 6,000 tokens; exit 3 signals a failed token bound and requires stopping the run series.
Installed adds exactly --disallowedTools Bash,Read,Grep,Glob,Edit,Write,NotebookEdit,Agent,WebFetch,WebSearch; it retains user settings, instructions, skills and hooks. No configuration file is edited.
