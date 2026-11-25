# The Academy memory domain

`ontology/academy-memory.schema.json` declares the domain `academy_memory`, version 1: what the teacher and the agent know about each student, and as of when. It is installed in the host's Ontology store beside the domain `work` and declares that domain's namespace. The test `crates/edu-academy-ontology/tests/academy_memory.rs` checks, installs and applies it with the engine this workspace pins.

**The Academy's server is the authority of the records.** This domain holds readings of them, each with its time: a binding with its `observed_at`, an observation with its `observed_at` and the server's `record_id`, and the student's `memory_as_of`, the last lesson date whose records the memory holds. A record the server holds after a student's `memory_as_of` has not been read yet: a reader asks the server for those records only. Nothing here is a copy of a record as an authority, and no action calls anything outside the store.

## Identity

`student_id` is the id by which the domains of `ontology/academy.schema.json` and `ontology/academy-read.schema.json` know the same student. This domain adds no second identity, only the slug the teacher's files use as the key. The types `registered_book`, `lesson_record` and `section_attempt` of `academy_read` are that domain's spool of the server's records, and they are not installed on the host. `textbook_binding` and `observation` are the teacher's and the agent's readings with their as-of time. Neither replaces the other. An observation carries the server's record id, so a later unit can link the two once that domain is installed.

This domain cannot share a store with `ontology/academy.schema.json`, which declares a type `student` of its own.

## Types

Every type is owned by the store and keyed by its native key, which the caller gives in the Id. The engine does not check that a key equals the properties it is made of; the caller builds it as stated.

The times `updated_at`, `observed_at` and `written_at` are UTC strings of the form `YYYY-MM-DDTHH:MM:SSZ`, at most 40 bytes. The caller of the action that writes a time sets it; the engine bounds its length only. A date (`iso_date`) is `YYYY-MM-DD`; a server key (`key_digits`) is 1 to 32 digits.

| Type | Native key | Properties (bytes; required in bold) |
| --- | --- | --- |
| `student` | the slug | **`slug`** (100), `name` (100), `student_id`, `course_id`, `membership_id` (key_digits), **`status`** (student_status), `class_slot` (100), `surface` (2,000: the few facts a lesson starts from), `memory_as_of` (iso_date), `updated_at` |
| `textbook_binding` | `<slug>\|<book_no>` | **`student`**, **`book_no`** (key_digits), `course_key` (40), `book_label` (600: word for word as the server names it), `role` (200), **`status`** (binding_status), `provenance`, `observed_at` |
| `observation` | given by the caller | **`student`**, **`kind`** (observation_kind), **`lesson_date`**, **`observed_at`**, `tool` (100), `record_id` (100), **`summary`** (1,500) |
| `lesson_note` | `<slug>\|<lesson_date>` | **`student`**, **`lesson_date`**, **`text`** (8,000), **`provenance`**, `written_at` |
| `reminder` | the reminder's id | `student` (none for a reminder of a whole class), **`text`** (600), **`status`** (reminder_status), `due` (iso_date), `provenance`, `updated_at` |
| `counseling` | `<slug>\|<date>` | **`student`**, **`date`** (iso_date), `purpose` (600), `form` (8,000), `result` (8,000), **`status`** (counseling_status), `stored_at` (300) |

A view argument may hold only ASCII letters, digits and `._:/-`. A slug is passed as the argument `student`, so it is written in those characters.

A lesson's daily record, a pre-study, its report, the section results, a daily test and a clinic are each an observation of their `kind` (`dailyrecord`, `prestudy`, `prestudy_report`, `section_results`, `dailytest`, `clinic`; also `metadata` and `other`), with the server's record id. A student's progress is the lesson notes together with the student's `memory_as_of`.

The value types:

- `provenance`: `server_observed`, `teacher_confirmed`, `local_state`, `planned`, `unconfirmed`.
- `student_status`: `onboarding`, `active`, `paused`, `transferred`, `archived`.
- `binding_status`: `active`, `candidate`, `retired`.
- `reminder_status`: `open`, `closed`.
- `counseling_status`: `prepared`, `held`, `closed`.

## Links

Each reference to `student` is a many-to-one link, named forward and reverse as follows:

| Link | Reverse from `student` |
| --- | --- |
| `binding_student` (`textbook_binding`) | `bindings` |
| `observation_student` (`observation`) | `observations` |
| `note_student` (`lesson_note`) | `lesson_notes` |
| `reminder_student` (`reminder`) | `reminders` |
| `counseling_student` (`counseling`) | `counselings` |

## Actions

A parameter is named like the property it sets; the object an action writes is the parameter of its type (`student`, `binding`, `observation`, `note`, `reminder`, `counseling`), a full Id. An optional parameter left out leaves the property as it was.

| Action | Parameters | Effect | Refusals |
| --- | --- | --- | --- |
| `register_student` | `student`, `slug`, `status`; optional the other properties of `student` | creates or modifies the student | none of its own |
| `set_surface` | `student`, `surface` | sets `surface` | `student_missing` |
| `bind_textbook` | `binding`, `student`, `book_no`, `status`; optional `course_key`, `book_label`, `role`, `provenance`, `observed_at` | creates or modifies the binding | `student_missing` |
| `retire_binding` | `student`, `binding` | sets the binding's `status` to `retired` | `student_missing`, `binding_missing` (the binding does not exist) |
| `record_observation` | `observation`, `student`, `kind`, `lesson_date`, `observed_at`, `summary`; optional `tool`, `record_id` | creates or modifies the observation by its key | `student_missing` |
| `write_lesson_note` | `note`, `student`, `lesson_date`, `text`, `provenance`; optional `written_at` | creates or modifies the note | `student_missing` |
| `open_reminder` | `reminder`, `text`; optional `student`, `due`, `provenance`, `updated_at` | creates or modifies the reminder with `status` `open`; without `student` it is a reminder of a whole class | `student_missing` (a named student does not exist) |
| `close_reminder` | `reminder`; optional `updated_at` | sets `status` `closed` | `reminder_not_open` (the reminder does not exist, or its status is not `open`) |
| `prepare_counseling` | `counseling`, `student`, `date`; optional `purpose`, `form`, `stored_at` | creates the counseling with `status` `prepared` | `student_missing`; an existing key is refused by the engine (`revision_conflict`) |
| `record_counseling` | `counseling`, `result` | sets `result` and `status` `held` | `counseling_missing` |
| `close_counseling` | `counseling` | sets `status` `closed` | `counseling_missing` |
| `close_lesson_day` | `student`, `lesson_date`, `updated_at` | sets `memory_as_of` to `lesson_date`, and `updated_at` | `student_missing` |

A refusal by a criterion is the engine's code `submission_criteria`, with the criterion's code in `failed`. For example, `record_observation` for a student that was never registered answers `submission_criteria` with `failed` `["student_missing"]`. Every action except `register_student` that names a student who does not exist is refused this way. A value of the wrong form or over its bound is `invalid_parameter`.

## Views

Each view is a table over one type. A view with the argument `student` takes the student's slug.

| View | Over | Rows | Columns |
| --- | --- | --- | --- |
| `student_card` | `student` | the student whose `slug` equals the argument | slug, name, status, class slot, student id, course id, membership id, memory as of, updated at, surface |
| `student_bindings` | `textbook_binding` | the student's bindings with `status` `active`, by book number | key, book number, course key, book label, role, provenance, observed at |
| `student_observations` | `observation` | the student's observations, newest `observed_at` first, at most 12 | observed at, kind, lesson date, tool, record id, summary |
| `student_notes` | `lesson_note` | the student's notes, newest `lesson_date` first, at most 3 | lesson date, provenance, written at, text |
| `student_reminders` | `reminder` | the student's reminders with `status` `open`, by key | key, text, due, provenance, updated at |
| `student_counselings` | `counseling` | the student's counselings, newest `date` first, at most 2 | date, status, purpose, stored at, form, result |
| `roster` | `student` | students whose `status` is `active` or `onboarding`, by key | key, name, class slot, memory as of |
| `open_reminders` | `reminder` | every reminder with `status` `open`, of a student or of a whole class, by key | key, student, text, due, provenance, updated at |

## Grants

There is one grant: the principal `agent`, with no role and scope `{}`, reads and derives every type and may apply every action.

## Not here

This domain has no reader restriction, erase rule or other security measure. Installing it on the host and importing the students are host steps that happen after the merge.
