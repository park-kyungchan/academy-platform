# Measurement harness (slice 1 repair)
`python3 run.py Q01|Q02|Q03 <model> --mode isolated|installed` runs exactly one session.
Both modes use a per-run strict MCP config, an empty working directory, stream-json with verbose output and --include-hook-events, and no session persistence.
`isolated` uses `--setting-sources "" --tools "" --disable-slash-commands --system-prompt "You are an assistant of a teacher. Use the tools you are given."`; existing CLI login is retained without reading or passing credentials.
`isolated` measures the server's own texts alone, with the fixed neutral system prompt and user setting sources disabled.
`installed` measures the same with the user's instructions, skills and hooks loaded, without file or shell tools.
Each temporary run directory holds mcp.json, run-log.jsonl, claude.jsonl, claude.stderr, answer.txt and measurement.json.
Tool names/inputs, the last Academy result matched by tool_use_id, answer, usage and cost come only from the stream; argument bytes use compact UTF-8 JSON. Other tools never supply the display reference.
`received_form` is envelope when received JSON has a string display (that field is the reference), otherwise display_text; no Academy result is unknown. `shown_unchanged` compares whitespace-normalised reference text; `extra_chars` counts normalised answer characters outside its first complete display occurrence (all answer characters if none). Missing evidence or an empty/whitespace-only reference is `unknown`.
Answer date/forbidden-date/order/line checks use checks.json; failed measurements never justify changing the server, list or displays.
Offline tests: `python3 -I -m unittest discover -s crates/edu-academy-teacher/harness`; overrides: EDU_ACADEMY_TEACHER_BINARY and CLAUDE_BIN.
The exact command and fixed system prompt are saved in measurement.json; no authentication environment is changed.
Isolation checks init fields tools, slash_commands, terminal_slash_commands, skills, plugins, agents and mcp_servers when present; missing tools/init/first-request usage fails closed.
Only the 14 configured Academy tools and CLI agents claude, Explore, general-purpose, Plan, statusline-setup, web-fetch are allowed. Every listed plugin must have path=builtin.
Non-builtin plugins, skills, slash commands or hook stream events fail isolation; all existing inspected init fields and values are recorded. Native memory_paths metadata is also recorded, but an auto-memory destination does not prove a file load. The first assistant message's input+cache-read+cache-creation must be under 6,000 tokens; exit 3 signals a failed token bound and requires stopping the run series.
Installed adds exactly `--tools Skill` alongside the per-run Academy allow list; it retains user settings, instructions, skills and hooks. No configuration file is edited.
Both modes request --include-hook-events; isolation records hook_condition as pass/FAIL only with that flag. Older streams give hook_condition=unknown and pass (hooks unknown) if the other isolation checks pass.
Offline re-scoring: python3 -B crates/edu-academy-teacher/harness/rescore.py <run-dir> [...]. It uses the same score_stream as live runs, reads evidence without modifying it, and emits only mode/request/model/received_form/shown_unchanged/facts_kept/extra_chars/isolation/delivery. No tool arguments, file contents, answers or reference text leave the saved directories.
Installed rows retain a reduced fail-closed check: exactly one init, well-formed stream, and a tools list limited to per-run Academy names plus Skill. Passing this check reports `not_applicable`; failure reports `FAIL` with reasons. Full isolation criteria apply to isolated rows. Hook evidence remains recorded.

Q03 requests preparation for the unknown class 가상반Z on scenario v2, triggering a class_name ask with the three known class labels. Checks require every label and exactly one Academy call before a teacher answer; Skill calls are counted separately in total calls.
`received_text_bytes` is the UTF-8 length of the last delivered Academy text before whitespace normalisation. Per-call lengths are matched by tool-use id in the stream, then by tool name and call order to the optional server run-log. `delivery` is `same` when all matched lengths equal display_bytes, `truncated_or_changed` on a mismatch, or `unknown` if evidence cannot be paired. Equal byte lengths do not prove equal content; shown_unchanged scoring still uses the delivered text.

Q01/Q02 previous_lesson_date: the assignment/record provenance names November 9; forbidden-date checks reject November 4 (ISO and Korean forms).
Q01 line0 and Q02 line1: A's 2-1 video row says uploaded and awaiting teacher confirmation, without server/teacher completion claims.
Q02 roster_order: complete student action blocks occur in roster order A, B, C (reminders stay in their separate section).
Q02 line0: B's video absence is scoped to the lists, never a claim the student did not do or upload the work.
Q03 labels and academy_calls_before_answer: show all three class choices and wait after one Academy call. The default fixture is scenario-v2.json.

`shown_unchanged` is pass when the complete whitespace-normalised delivered text occurs in the final answer; missing evidence is unknown (the stored legacy `verbatim` alias remains).
`facts_kept` is pass when every row keeps its fields in order, every word-table state-label count is unchanged, every unknown-section line is kept, and at most one unanchored line is added; missing/invalid evidence is unknown.
A slice is accepted on `facts_kept`; `shown_unchanged` is reported beside it. Global row order, title, footer and closing retention are measured but do not decide `facts_kept`.
Q01/Q02 use checklist scoring; Q03 asks have no rows, so require the complete delivered text instead of vacuously passing. `checks.json` declares each request's `fidelity_kind`.
Offline counts: `python3 -I -m unittest discover -s crates/edu-academy-teacher/harness`; `python3 crates/edu-academy-teacher/harness/fidelity.py <run-dir>` prints counts/categories only, never answer text.
Fields use literal whitespace-normalised matches; nonrow/addition anchors remove Markdown syntax. Counts cannot verify student attribution or added-sentence meaning; a field can be reused across source rows.
