# Academy lesson archive

User decision, 2026-11-06: the academy server removes older rows after a while, so later
lessons cannot always re-read them. The shared Hindsight bank `coding-agent::shared` is the
durable archive of Academy lesson records. This protocol is the same in every CLI (Muse,
Claude Code, Codex, Antigravity CLI): it uses only native file editing and the Hindsight MCP
tool `ingest_document`, which all four CLIs register. It needs no hook, and it does not depend
on session retention.

## What to archive

- **Teacher statements.** Record each one verbatim with attribution and time, for example:
  `14:43 KST teacher: "1단원 14/16이고 2단원은 13/16이며 오답까지 전부 완료했다"`. These cover
  completions, judgments, corrections, assignments and plans. Mark a plan as a plan.
- **Source observations.** Section scores, per-item O/X and wrong item numbers, DailyTest rows,
  prestudy rows, print codes, paperKey/attemptKey, the dates as the source shows them, and the
  observation time in UTC with the tool name.
- **Saved DailyRecord values.** Record them only after an independent readback.
- **Tool failures.** Record the exact code, for example `printed_paper_info_fields_rejected`.
- **Student identity.** Use real student names (user decision 2026-11-06). Native student IDs are
  allowed when they help a later join.

Never archive credentials, tokens or cookies; answer-key literals (for example
`academy_item_answer_lookup` results); or parent contact details.

## Possible mis-reference by the teacher

The teacher sometimes names the wrong object by mistake. Observed cases: on 2026-11-02 the teacher
wrote Davinci `5-1-2` for `5-1-1` and corrected it ("내가 잘못 입력했다"); on 2026-11-02 the teacher
confirmed Gauss `5-1-2` rather than `5-2-2`; on 2026-11-06 `학원서버 프린트` meant a printed chapter
assessment, not the server unit-assessment row. Rules:

1. If the teacher's referent (student, book or volume, unit number, activity or assessment kind,
   date) conflicts with the records, or [term-map.md](term-map.md) lists several candidates for it,
   ask one short confirmation that shows the candidates. Do not silently follow the words, and do
   not silently substitute the record's value.
2. Record the teacher's original words, the confirmed referent and the confirmation time.
3. A later teacher correction supersedes the earlier entry for the same occurrence. Keep the
   earlier entry as dated history.

## Completion wording is not mapped

Keep the teacher's exact words: `풀이 완료`, `오답까지 완료`, `검토 완료`, `완료판정` and so on.
The user decided on 2026-11-06 not to map one completion phrase onto another, because the
mapping confused both the teacher and the agents. Never convert a source score or a moved
prestudy row into a completion. Ask the teacher when a decision depends on it.

## Step 1 — during the lesson: the local day log

When the teacher gives a decision or says `기록해라`, append one entry to the day log:

`/home/palantirkc/work/edu-lessons/archive/<YYYY-MM-DD>-session<N>.md`

Create the directory with mode 0700 and the file with 0600 if they are missing. Use the CLI's
own file tool. The business date is the Asia/Seoul date. Entry format:

`- <HH:MM KST> [teacher | source:<tool> observed <HH:MM UTC> | agent-report] <student name> — <statement>`

Say `기록했습니다` only after the write succeeded and a read of the file shows the entry. Write
the prose in English and keep Korean labels and teacher quotes verbatim.

## Step 2 — once, after the lesson ends: ingest the whole day log

Send to Hindsight once, after the lesson has ended: when the teacher says so, or when the teacher
confirms the agent's question "수업이 끝났으면 Hindsight에 보낼까요?". Do not send during the
lesson. If a session ends mid-lesson, the local log stays, and the next session continues the same
log. The only re-send is a later correction to that day, which replaces the same document.

Call the Hindsight MCP `ingest_document` with:

- title exactly `Academy lesson archive <YYYY-MM-DD> session<N>`
  (document id `academy-lesson-archive-<yyyy-mm-dd>-session<n>`)
- content = the complete current day log

The title fixes the document id. A same-id retain uses the server default
`update_mode: replace`, so send the whole file every time, never a fragment. Run the call from
`/home/palantirkc/work/edu-lessons` so that the stamped tag is `project:edu-lessons`. Verify
that the returned `doc_id` matches, then check `sync_status` for pending work.

A correction to an earlier day goes into that day's log, which is then re-ingested. Add a one-line
pointer to the correction in today's log.

## Reading the archive

Search Hindsight with the student's real name plus the book, unit and activity. Use
[term-map.md](term-map.md) for the English and Korean variants. Each archived observation is a
**dated snapshot**: if the student re-solves a paper or the teacher corrects a score later, the
server changes but the snapshot keeps the old value. That is what "stale" means. Rules:

1. While the server still shows the row, a fresh Academy read wins. The archive explains history.
2. Once the server no longer shows the row, the archived snapshot is the record. Cite its date.
3. Between two snapshots, or a snapshot and a teacher statement, the later one wins for the same
   occurrence. Keep the earlier one as dated history.
4. A missing archive entry does not prove that the work was not performed.
