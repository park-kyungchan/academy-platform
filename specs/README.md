# Specifications

A specification here is the contract of one command or one option: the behaviour that its
callers rely on and that its tests check. Code is built and tested against it; a reviewer
compares the two. It states behaviour that can be observed from outside (arguments, output,
exit codes, files written or left alone), never how the code is organised: no function,
type or module is a rule.

| Specification | Contract of |
| --- | --- |
| [edu/lesson.md](edu/lesson.md) | The lesson command `edu lesson` (crate `crates/edu`) |
| [academy-mcp/paper-admit.md](academy-mcp/paper-admit.md) | The option `--paper-admit` of `modules/academy-workbench/tools/academy-mcp/install/loop_install.py` |
| [academy-mcp/tool-surface.md](academy-mcp/tool-surface.md) | The tool surface of `academy_operations`: `release/surface.json` and `content/skills/academy-*/SKILL.md` |
| [academy-mcp/direct-key-lookup.md](academy-mcp/direct-key-lookup.md) | The direct-key historical fallback of `academy_textbook_section_results_get` and the read-only `ReportViewServlet` views |
| [academy-mcp/lms-reads.md](academy-mcp/lms-reads.md) | The read-only LMS reads of `academy_lms_schedule_get` and `academy_prestudy_get` |
| [academy-mcp/gated-writes.md](academy-mcp/gated-writes.md) | The two-phase writes `academy_clinic_paper_{review,save}` and `academy_exam_paper_{review,save}` |
| [studyq-linux/core.md](studyq-linux/core.md) | The Linux StudyQ executor `python3 -m studyq_linux` (`render`, `assemble-print`) |

## The form of a specification

One Markdown file under `specs/<area>/<name>.md` with these sections, in this order:

1. `## Purpose`: what the contract is for and who relies on it.
2. `## Requirements`: one line per rule, `- R<n>: <rule> Tests: <test>, <test>.`, numbered
   from 1. A rule is one behaviour that a test can break. Each rule names the tests that
   check it, by the test function's name in the test file the section Interface names, or
   is marked `(no test yet)`, optionally with what will observe it.
3. `## Interface`: the forms, members, codes, files and limits the rules speak of, and the
   test file.
4. `## Not included`: what a reader could expect here and the contract does not promise.

A specification may add a section that a rule refers to (for example a table of fields);
it comes before `## Not included`.

## Rules

- A requirement is cited as `specs/<area>/<name>.md#R<n>`. A number is never reused or
  renumbered: a rule that ends is replaced by the line `- R<n>: (removed)`, and a new rule
  takes the next number.
- A pull request that changes a behaviour a specification states changes that
  specification in the same pull request, and changes or adds the test that the old
  behaviour fails.
- A test that checks a rule is named in that rule; a rule that no test checks says
  `(no test yet)`.
- Limits are written in units (5 minutes, 1 MiB, 64 KiB), not as long runs of digits.
- No student name, record text, student id, account name or credential appears in a
  specification. An example uses invented values and says that it is an example.
