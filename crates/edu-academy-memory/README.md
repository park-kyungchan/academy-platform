# edu-academy-memory

One command that reads and writes a student's memory in the domain `academy_memory` (`ontology/academy-memory.schema.json`, described in `docs/academy-memory.md`). The command doesn't open the store file and doesn't call the Academy's server. It runs the host's engine as a process: the program in `EDU_ONT_BIN`, or `/usr/local/bin/ont` when that variable is unset. `--store <path>`, given before the subcommand, is passed to the engine, so the command can run on a scratch store.

Every value below is invented.

## Reads

| Subcommand | Example | Answer |
| --- | --- | --- |
| `brief <slug>` | `edu-academy-memory brief stu-a` | text of at most 6,000 bytes in six sections (see below) |
| `show <type> <key>` | `edu-academy-memory show observation 'stu-a\|dailytest\|2040-01-02\|2040-01-02T09:00:00Z'` | one object whole, as text: `<type> <key>`, `head: <revision>`, then `<property>: <value>` per property |
| `roster` | `edu-academy-memory roster` | one line per active or onboarding student: `<slug>\tclass_slot=<v>\tmemory_as_of=<v>` (`none` when absent) |
| `due <slug> <lesson_date>` | `edu-academy-memory due stu-a 2040-01-02` | `{"student","lesson_date","memory_as_of","have":[{"kind","observed_at"}],"due":[kinds],"bindings":[{"book_no","course_key","book_label"}]}` |

`brief` reads everything through the engine's `search`. It follows every continuation and sorts the results itself; it never uses a view. Each section has its own byte budget, which covers the separating blank line, the heading, every entry line and the section's last line:

| Section | Budget | Entry line at most | Entry |
| --- | --- | --- | --- |
| `## student` | 1,000 | what is left | `slug`, `name`, `status`, `class_slot`, the three server ids, `memory_as_of`, `updated_at`, `surface` |
| `## bindings (active)` | 700 | 160 | `- book_no=<n> course_key=<k> label=<label>`, in book-number order |
| `## observations (12 newest)` | 2,400 | 180 | `- <kind> <lesson_date> <observed_at>: <summary>` |
| `## notes (3 newest)` | 1,200 | 370 | `- <lesson_date> <provenance>: <text>` |
| `## reminders (open)` | 400 | 110 | `- <id> (student\|class)[ due <date>]: <text>`: the student's own reminders and those of the whole class |
| `## counselings (2 newest)` | 300 | 120 | `- <date> <status>: <purpose>` |

In a brief, a control character (a line break, for example) is printed as a space. A text over its bound is cut at a character boundary and ends `… (+<n> bytes)`. `show` prints the whole text. An observation's key is `<slug>|<kind>|<lesson_date>|<observed_at>`, so it can be built from the fields its line prints; a binding's whole label is also in `due`. An entry that doesn't fit in what is left of its section is left out together with every entry after it, and the section's last line is then `(+<k> more)`. A section with no entries prints `none`.

`due` reads every observation of the student and date with a search that has both equalities. `due` lists the kinds among `dailyrecord`, `prestudy`, `section_results`, `dailytest` and `clinic` that have no observation for that date. It also lists the active bindings with their exact keys.

## Writes

| Subcommand | Example | Action |
| --- | --- | --- |
| `student <slug> --name <text> [--student-id <d> --course-id <d> --membership-id <d> --status <s> --class-slot <text>]` | `edu-academy-memory student stu-a --name 'Invented Student' --status active --class-slot slot-1` | `register_student` |
| `surface <slug> --text <text or @file>` | `edu-academy-memory surface stu-a --text @surface.md` | `set_surface` |
| `bind <slug> <book_no> --label <text> [--course-key <k> --role <text> --status active\|candidate --at <utc>] --provenance <p>` | `edu-academy-memory bind stu-a 1234 --label 'Invented Book 2-1' --course-key CK-12 --at 2040-01-02T08:00:00Z --provenance server_observed` | `bind_textbook` |
| `unbind <slug> <book_no>` | `edu-academy-memory unbind stu-a 1234` | `retire_binding` |
| `observed <slug> <kind> <lesson_date> --at <utc> --summary <text or @file> [--tool <t> --record <id>]` | `edu-academy-memory observed stu-a dailytest 2040-01-02 --at 2040-01-02T09:00:00Z --summary '8 of 10' --record rec-1` | `record_observation` |
| `note <slug> <lesson_date> --provenance <p> --text <text or @file>` | `edu-academy-memory note stu-a 2040-01-02 --provenance teacher_confirmed --text @note.md` | `write_lesson_note` |
| `remind open <id> --text <text> [--student <slug> --due <date>] --provenance <p>` | `edu-academy-memory remind open r-1 --text 'bring the invented sheet' --student stu-a --due 2040-01-09 --provenance planned` | `open_reminder` |
| `remind close <id>` | `edu-academy-memory remind close r-1` | `close_reminder` |
| `counsel prepare <slug> <date> --purpose <text> --form @<file> [--stored-at <text>]` | `edu-academy-memory counsel prepare stu-a 2040-01-05 --purpose 'invented purpose' --form @form.md --stored-at counsel/stu-a.md` | `prepare_counseling` |
| `counsel result <slug> <date> --result @<file>` | `edu-academy-memory counsel result stu-a 2040-01-05 --result @result.md` | `record_counseling` |
| `counsel close <slug> <date>` | `edu-academy-memory counsel close stu-a 2040-01-05` | `close_counseling` |
| `day-close <lesson_date> <slug>...` | `edu-academy-memory day-close 2040-01-02 stu-a stu-b` | `close_lesson_day` for each student |

Every write prints `{"action","key","outcome"}`. Before applying, a write reads its object with `get`; for `get`, the refusal `unavailable` means the object doesn't exist. If the object already holds every value the call would set, the command applies nothing and the outcome is `already_applied`. Otherwise the request id is `eam-` followed by 60 hexadecimal digits of the SHA-256 of the action, its parameters (an `@file` argument by the file's contents) and the object's head revision, or `new` when there is no object. So running the same call twice changes nothing the second time, and setting a state again after another state was set applies. `student` without `--status` passes the stored status; for a new student, `--status` is required. `bind` without `--status` passes `active`. `stored_at` is set only by `counsel prepare`, because `prepare_counseling` is the only action that takes it.

`day-close` refuses a student with `nothing_read` when the store holds neither an observation nor a note of that student and date. For every other student it applies `close_lesson_day` with the clock's UTC time as `updated_at`. That time is left out of the comparison and of the request id. It prints one line per student on standard output, either `{"action","key","outcome"}` or `{"error":{...}}`, and exits 1 when any student was refused.

## Import

`edu-academy-memory [--store <path>] import <directory> [--dry-run]` turns the memory the teacher's sessions wrote as Markdown into store objects, and can be run again: a second run of an unchanged directory creates nothing and moves no object's revision. It only reads the directory; the directory is an argument and no path is built into the command. It writes through the writes above (`student`, `surface`, `bind`, `unbind`, `remind`, `note`, `day-close`), each of which applies nothing where the object already holds the values.

Under `<directory>/students` it reads:

| File | Read |
| --- | --- |
| `REGISTRY.md` | The table with the header `Slug`, `Student Name`, `studentId`, `courseId`, `membershipId`, `Status`, `Class Slot`, `Directory`: one student a row (`register_student`; the status in lower case; an id cell of `-` or empty is left out). A line before the table with a reminder id and the word `OPEN` or `CLOSED` is a reminder of the whole class: its text is the line without the id and the word (and the brackets or marks that are empty without them), its provenance `local_state`. |
| `<slug>/profile.md` | The table with the columns `ID`, `Status`, `Summary`, `Provenance`: one reminder a row (`remind open`, then `remind close` when the status is `CLOSED`); the provenance is the first tag of its cell, in lower case. A reminder id is `R-YYYYMMDD-<KEY>` (capitals, digits, `-`) or `R-YYYYMMDD-NNN`, at most 32 bytes. |
| `<slug>/textbook-ledger.md` | Every table with a `bookNo` column; the rows of one book number are merged before any write. The label and the course key come from a table with `Verbatim bookLabel` when there is one, else from `Book Name` and `courseKey`; the role (`Role / Track`), the provenance (first tag), the status and the time (`Observed / Confirmed At`, kept only when it is a UTC time) come from the first row that has a value. Status `ACTIVE` binds; a status that contains `CANDIDATE` binds with status `candidate`; any other binds and unbinds. |
| `<slug>/progress-log.md` | Each section headed `## YYYY-MM-DD`, with or without text after the date (heading line included), is a note of that date with the provenance `local_state`, its text the section word for word, trailing blank lines left out. Two sections of one date are joined in file order with an empty line. A text over 8,000 bytes is cut at a character boundary and ends with the line `(cut)`, 8,000 bytes at most. Other `##` headings are not read; a heading that begins like a date but is none is a parse entry. After the notes `day-close` is applied with the newest note's date; a student with no dated section gets none, so its `memory_as_of` stays `none` in `roster`. |
| `<slug>/CONTEXT.md` | The bullets (and their indented continuation lines) under the heading that contains `Current Surface`, up to the next heading of the same or a higher level: `surface`, cut at 2,000 bytes. |
| `<slug>/hindsight-staging.jsonl` | Not imported; only its lines are counted. |

A table is a Markdown table with a leading `|`; `\|` in a cell is a `|`; fenced blocks are skipped. The students imported are the rows of the registry. A row whose status is not a value of the schema, or whose slug, name or ids are not valid, is a parse entry and the student is skipped whole. A row whose directory `students/<slug>` is missing is registered and is a parse entry. A directory under `students/` without a row, but for `_template`, is a parse entry and is not read. A file that does not exist is not an error; a file that cannot be read as UTF-8 is a parse entry. A reminder id that appears twice in the run is imported once and the second is a parse entry. A book with no label, no provenance or no status, a row with no `bookNo` or one that is not 1 to 32 digits, a reminder with an unknown status, no summary or no known provenance are parse entries.

For a pair (`bind` then `unbind`, `remind open` then `remind close`) the import first reads the object: when it already holds the pair's final state with the same values, it writes neither and counts one `already_applied`. A pair that writes counts one `created`.

A real run prints one JSON object on standard output:

```json
{"students": {"<slug>": {"created": 0, "already_applied": 0, "refused": 0, "staging_lines": 0}},
 "class_reminders": {"created": 0, "already_applied": 0, "refused": 0},
 "parse": [{"path": "students/<slug>/profile.md", "line": 7, "reason": "..."}]}
```

`path` is relative to the directory and `line` is 1-based (0 for a file or a directory as a whole). Nothing a run can read is held back by another row's failure. A refusal by the engine is counted and one line `{"refused":{"owner","action","code","field"}}` goes to standard error, with no text of the record. The exit is 0 when `parse` is empty and nothing was refused, else 1; an engine that cannot be run is `engine_missing`, exit 3, as everywhere. `--dry-run` parses the directory, builds the same list of writes and applies none; it does not call the engine, and prints `{"students": {"<slug>": {"planned": {"<action>": <count>}, "staging_lines": 0}}, "class_reminders": {"planned": {}}, "parse": []}`, with exit 0 when `parse` is empty, else 1. A directory without `students/REGISTRY.md` or without a table of students, a missing directory argument and an unknown option are refused (`invalid_input` or `usage`, exit 2).

## Input and refusals

- A slug and a reminder's id are 1 to 32 bytes of ASCII letters, digits, `.`, `_` and `-`.
- A date is `YYYY-MM-DD`.
- `--at` takes only `YYYY-MM-DDTHH:MM:SSZ`.
- A `book_no` is 1 to 32 digits.

Input that breaks these rules is refused with `invalid_input` before any call to the engine.

A refusal is one line on standard error, `{"error":{"code","field","detail"}}`, with exit 2. The code is the engine's own when the engine refused (for example `submission_criteria`, `invalid_parameter`, `revision_conflict`, `unavailable`). Otherwise it is one of the command's own codes: `usage`, `invalid_input`, `nothing_read` or `student_missing`. `student_missing` is used by `brief`, `due`, `show student` and `day-close` for a slug that has no student. When the engine can't be run, or answers something that isn't its JSON, the code is `engine_missing` with exit 3. `day-close` exits 1 in place of 2.

## Tests

- `tests/cli.rs` runs the command against a stand-in engine of the test's own, so no engine is needed. The stand-in is a script in the test's temporary directory; where that directory doesn't allow executing files, the test runs its own executable instead.
- `tests/fixtures/import-class` and `import-clean` are invented workspaces that hold every form the import reads; the tests whose names begin with `import` run them against a fake store the stand-in keeps in a file, so a second run can be compared with the first.
- `tests/real_engine.rs` is ignored by default. It runs every subcommand against a real engine on a scratch store in its own temporary directory:

```sh
EDU_ONT_BIN="$(command -v ont)" cargo test --locked -p edu-academy-memory --test real_engine -- --ignored
```
