# Direct-key historical lookup of textbook results: `academy_textbook_section_results_get`

## Purpose

`academy_textbook_section_results_get` finds a student's textbook through the live list
`getTextBookProgress`, which holds only the books in progress: an archived textbook or exam
(an old book number) is absent from it, so its results cannot be read today although the LMS
still answers for its keys. This contract adds a read-only fallback that takes the keys of
such a book from the `edu-academy-memory` bindings or from explicit verified parameters and
reads the results directly. It also fixes the read-only report views of `ReportViewServlet`
and the account-lock rule that every Academy read of the TypeScript server follows. It is
relied on by the teacher's lesson and counseling sessions, by the tool surface
([tool-surface.md](tool-surface.md)), and by the units that build and check the fallback.

## Requirements

- R1: When the live `getTextBookProgress` list holds the requested book, the tool answers as before and its answer has no `bindingSource` of `historical_key_registry`. (no test yet; observed by the unit that builds the direct-key fallback)
- R2: When the live list does not hold the requested book, the tool takes `(courseKey, bookNo, studentKey)` from the student's active bindings of `edu-academy-memory` (the lines `- book_no=<n> course_key=<k> ...` of `## bindings (active)`, as `due` lists them under `bindings`) or, when the caller gives all three, from the explicit parameters `courseKey`, `bookNo` and `studentKey`. (no test yet; observed by the unit that builds the direct-key fallback)
- R3: With those keys the tool sends only the read-only requests `getWrongAnswerNoteResult` and `getCourseBookReport` of the Interface, and answers the section results with `bindingSource: 'historical_key_registry'`. (no test yet; observed by the unit that builds the direct-key fallback)
- R4: Keys are verified, never guessed: the course, book and student keys that the answered report page carries must equal the keys sent, or the call refuses with `historical_key_unverified` and answers no row. (no test yet; observed by the unit that builds the direct-key fallback)
- R5: A book that neither the live list, an active binding nor explicit parameters give refuses with `historical_binding_missing`; two active bindings of the same `book_no` with different `course_key` refuse with `historical_binding_ambiguous`; neither sends a request after the live list. (no test yet; observed by the unit that builds the direct-key fallback)
- R6: A key that is not 1 to 32 ASCII digits refuses with an invalid input before any LMS request. (no test yet; observed by the unit that builds the direct-key fallback)
- R7: The fallback reads `edu-academy-memory` only: it never binds, unbinds or writes an observation. (no test yet; observed by the unit that builds the direct-key fallback)
- R8: The report views of `ReportViewServlet` that a tool may read are exactly `SmartBookReportView`, `DiagReportView`, `NewStudentReportView`, `FaView`, `GaReportView` and `NaScoreResultView`, each by GET; any other view name refuses with an invalid input before any request. (no test yet; observed by the unit that builds the direct-key fallback)
- R9: I1 `write_forbidden`: under a mock transport that answers `write_forbidden` to every request whose route is classed write (by its named route and operation, never by its method alone, as [tool-surface.md](tool-surface.md) "Request classes" states) or that is not in the read list of the Interface, every call of R2 to R8 on the synthetic fixtures completes or refuses with its own code, and the transport records 0 `write_forbidden` requests. (no test yet; observed by the unit that builds the direct-key fallback)
- R10: Account lock: before its first LMS request a call of the TypeScript server takes the shared account lock of `docs/academy-account-lock.md`, waiting up to 45 seconds, and holds it until its last request; when the lock is still held after 45 seconds the call ends with a tool error carrying `owner_account_busy`, and 0 HTTP requests are sent. A Rust owner that finds the lock held fails at once with `owner_account_busy`, and `edu lesson` retries for up to its `lockWaitSeconds`, as that document states. (no test yet; observed by the unit that builds the direct-key fallback)
- R11: A page that is the LMS login page, a session-error page or a redirect to either refuses the call with `authentication_required` and answers no row (fail closed). (no test yet; observed by the unit that builds the direct-key fallback)
- R12: No error and no log line of the fallback holds a student name, a record text or a key value. (no test yet; observed by the unit that builds the direct-key fallback)

## Interface

### Requests (the read list)

| Read | Request |
| --- | --- |
| live list | `POST /servlet/controller.coursemanage.TextBookManageServlet?reqCmd=getTextBookProgress`, body `student_name=<name>` (the request the tool sends today) |
| the live path as built | every other request the tool sends today when the live list holds the book, as its existing tests' fixture transports record them (the login exchange, the paper search `p_process=UserBySearchTestResult` of `TestPageListServlet`, and the single-user result `p_process=getStudyResultSingleTestingSingleUser` of `TestpageSelectExServlet`) |
| wrong-answer note | `POST /servlet/controller.common.TestpageSelectExServlet`, body `p_process=getWrongAnswerNoteResult` with `answerNoteResult` naming the one course, book and student |
| course-book report | `POST /servlet/controller.coursemanage.TextBookManageServlet`, body `reqCmd=getCourseBookReport&course_key=%u&book_no=%u&student_pri_no=%u` |
| report views | `GET` of `ReportViewServlet` with one view of R8; its full path is pinned by the unit that builds it |

Every POST of this list is a read: it changes no Academy data. The read list is the whole set that the mock transport of R9 allows; R1 keeps the live path's requests in it, so the unchanged live path passes R9. An example of the binding line, with
invented values: `- book_no=1234 course_key=56 label=Invented Book 2-1`.

### Answer

The members the tool answers today, plus `bindingSource`: `historical_key_registry` when the
keys came from R2.

### Codes

| Code | When |
| --- | --- |
| `historical_key_unverified` | R4 |
| `historical_binding_missing`, `historical_binding_ambiguous` | R5 |
| `owner_account_busy` | R10 |
| `authentication_required` | R11 |

### Account lock

The shared lock file and its order are those of `docs/academy-account-lock.md`; in Rust the
lock is taken by `OwnerRoot::account`, which does not wait. The TypeScript server's installed
endpoint command takes the same file with `flock --wait 45`
(`modules/academy-workbench/tools/academy-mcp/src/upstream-process.ts`), so it waits up to 45
seconds. The path is host configuration and is not in this repository.

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/direct-key-lookup.test.ts`, with invented
keys and synthetic pages only.

## Not included

- Discovering the keys of a book that no binding and no caller names.
- Complete history: the fallback answers what the two reads answer for the keys.
- Changing the bindings of `edu-academy-memory`; that is its own `bind` and `unbind`.
- The view pages' members beyond their being read-only.
