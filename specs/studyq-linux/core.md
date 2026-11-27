# The Linux StudyQ executor: `python3 -m studyq_linux`

## Purpose

The Academy's official report PDFs and print PDFs are made today by Windows programs
(`StudyQReport.exe`, `CreateClinic`, `CreateTotalPrint`). `studyq_linux` makes the same PDFs on
Linux from one JSON document, with the official templates and font of a pinned asset
manifest, and fixes three known defects of `StudyQReport.exe`. It is one-shot: one run, one
PDF, one line of answer. It is relied on by the TypeScript Academy server's tools
`academy_official_report_pdf_get` and `academy_paper_print_pdf_get`
([tool-surface.md](../academy-mcp/tool-surface.md)), by the units that build and check the
executor, and by every later change of it. The rules were read from the decompilation of the
Windows programs; that decompilation is not part of this repository.

## Requirements

Form and answer

- R1: `render --type <type> --assets <dir> --out <pdf>` reads one UTF-8 JSON object from standard input and, when it succeeds, writes the PDF at `<pdf>`, ends with exit status 0 and prints exactly one line on standard output: a JSON object with exactly the members `status` (`"ok"`), `reportType` (the `<type>` given), `pages` (the number of pages of the PDF, at least 1), `sha256` (64 lowercase hex digits, the SHA-256 of the bytes at `outputPath`) and `outputPath` (the absolute path of `<pdf>`). (no test yet; observed by the unit that builds studyq-linux)
- R2: A run that fails ends with a non-zero exit status and prints exactly one line on standard output, `{"status":"error","code":<code>,"detail":<text>}`, `<code>` one of the table "Error codes"; no file is left at `<pdf>`, and a file that was there before the run is unchanged. (no test yet; observed by the unit that builds studyq-linux)
- R3: The words are one of the two forms of the Interface: an unknown verb, a missing, repeated or unknown flag, or a positional word refuses with `usage_invalid`, before standard input is read. (no test yet; observed by the unit that builds studyq-linux)
- R4: Standard input that is not valid UTF-8, larger than 4 MiB, not exactly one JSON object, or that lacks a member the type requires, holds a member the type does not take, or holds a member of another JSON type, refuses with `input_invalid`. (no test yet; observed by the unit that builds studyq-linux)
- R5: `<type>` of `render` is `course_book` or `cism`; any other value refuses with `type_unknown`. (no test yet; observed by the unit that builds studyq-linux)
- R6: An existing `<pdf>` refuses with `output_exists`; the PDF is written beside `<pdf>` under another name and renamed to `<pdf>` only when complete, so `<pdf>` is never seen partly written. (no test yet; observed by the unit that builds studyq-linux)
- R7: `detail` and standard error hold no value of the input (no student name, record text, student id or key) and no archive key; `detail` is at most 256 bytes. (no test yet; observed by the unit that builds studyq-linux)
- R8: The same input with the same assets gives the same PDF bytes, so the same `sha256`: the PDF holds no creation or modification time and no random identifier. (no test yet; observed by the unit that builds studyq-linux)

Assets

- R9: Before it reads a template, `render` checks the asset manifest in `<dir>`: a missing `report.zip` or `NanumGothic-Regular.ttf` refuses with `asset_missing`, and a file whose SHA-256 differs from its manifest value refuses with `asset_hash_mismatch`; neither writes a PDF. (no test yet; observed by the unit that builds studyq-linux)
- R10: The executor never creates, changes or removes a file under `--assets`, and opens no network connection. (no test yet; observed by the unit that builds studyq-linux)
- R11: `report.zip`, the templates inside it and the print archives are never committed to Git: no file of the repository has the SHA-256 of `report.zip` or of a template inside it. On the host they are installed read-only under `/usr/local/lib/academy-mcp/studyq-assets/`. (no test yet; observed by the unit that installs studyq-linux)
- R12: The font of every page is `NanumGothic-Regular.ttf` of the manifest, embedded in the PDF; no other font is embedded. (no test yet; observed by the unit that builds studyq-linux)

`course_book`

- R13: A `course_book` PDF draws its pages on the templates `coursebook_1.jpg` to `coursebook_7.jpg` of `report.zip`: pages 1 to 6 on templates 1 to 6, one page each, then template 7 for as many pages as its rows need, at least one. The full report (every section filled, the parity fixture) has 8 pages. (no test yet; observed by the unit that builds studyq-linux)
- R14: On page 4 each value is printed in the column whose heading names its input member, not in the swapped columns of `StudyQReport.exe`. (no test yet; observed by the unit that builds studyq-linux)
- R15: The chart is drawn at the origin of its box on the template, without the 36 px offset of `StudyQReport.exe`. (no test yet; observed by the unit that builds studyq-linux)
- R16: A line-break token of the input (Interface, "Line-break tokens") breaks the line and is not printed; no raw `rn` token appears in the text of any page. (no test yet; observed by the unit that builds studyq-linux)
- R17: The parity check of the Interface, run on the synthetic full `course_book` fixture, reports 0 mismatches and 0 raw `rn` tokens. (no test yet; observed by the unit that builds studyq-linux)

`cism`

- R18: A `cism` PDF draws on the templates of the table "cism templates" only: pages 1 to 3 on `cism_1.jpg` to `cism_3.jpg`, then one page for each of the groups 4, 5 and 6 in that order, on the variant `<group>_<k>.jpg` whose `k` is the number of filled `block%d` fields of that group; a `k` that the table does not hold refuses with `input_invalid`. (no test yet; observed by the unit that builds studyq-linux)
- R19: The `cism` input takes the block-indexed fields `block%d` (`%d` from 1), `week_no` (an integer from 1 to 53), `chart_data` (an array) and `report_list` (an array); each value is printed in the box of its field on its page, and the parity check reports 0 mismatches on the synthetic `cism` fixture. (no test yet; observed by the unit that builds studyq-linux)

`print.zip`

- R20: `assemble-print --out <pdf>` reads one JSON object from standard input naming `kind` (`exam`, `examtext` or `clinic`) and its keys, opens the archives that the path templates of the Interface give for those keys, and writes one PDF with the pages of the archives in the order of the input's list and, within an archive, in the order of its member list; it answers as R1 with `reportType` `print_<kind>`. (no test yet; observed by the unit that builds studyq-linux)
- R21: An archive path is made only from the templates of the Interface with integer keys: a key that is not an integer from 0 to 2^32 - 1, or a `d6`, `d4` or `d2` value outside the range its width allows, refuses with `input_invalid`, and no path is made from any other input text. (no test yet; observed by the unit that builds studyq-linux)
- R22: A `clinic` input holds exactly `clinic_list`, `exam_key`, `content_list`, `clinic_type` and `lecture_value`; the list members are strings of elements joined by `|,|`, and an empty element, or lists of different element counts where the Interface pairs them, refuses with `input_invalid`. (no test yet; observed by the unit that builds studyq-linux)
- R23: The archive key is read from the private host file that the environment variable `STUDYQ_PRINT_KEY_FILE` names (a regular file of the caller, not readable by group or others); it is never read from standard input, an argument or the repository, and is never printed or written. A missing variable or file, or a file of another owner or mode, refuses with `archive_key_missing` before any archive is opened. (no test yet; observed by the unit that builds studyq-linux)
- R24: An archive member whose name is absolute, holds a `..` component or a drive letter, resolves outside the extraction directory, or is a symbolic link refuses the whole run with `archive_refused` before any member is extracted. (no test yet; observed by the unit that builds studyq-linux)
- R25: An archive with more than 4096 members, a member that decompresses to more than 64 MiB, or members that decompress to more than 256 MiB together refuses with `archive_refused`; the limits count the bytes actually decompressed, not the sizes the archive declares, and decompression stops at the first byte over a limit. (no test yet; observed by the unit that builds studyq-linux)

Checks

- R26: The pure-Python tests (words, input schema, asset manifest, path templates, ZipSlip, size limits, the parity of the field tables) run in `./check` unconditionally, with no asset and no PyMuPDF. (no test yet; observed by the unit that builds studyq-linux)
- R27: The rendering tests run when `CHECK_STUDYQ_PYMUPDF=1` is set or the module `fitz` can be imported, and otherwise print one skip line and pass; the parity check against the official templates runs only when an assets directory is given, and otherwise prints one skip line. (no test yet; observed by the unit that builds studyq-linux)
- R28: `./check` fails when a file under `specs/` or a synthetic fixture of the executor holds a student name, a real student id, record text, a credential or an archive key (privacy check I6). (no test yet; observed by the unit that builds studyq-linux)

## Interface

### Forms

```
python3 -m studyq_linux render --type <type> --assets <dir> --out <pdf>   < input.json
python3 -m studyq_linux assemble-print --out <pdf>                         < input.json
```

An example answer, with invented values:

```json
{"status":"ok","reportType":"course_book","pages":8,"sha256":"<64 hex>","outputPath":"/tmp/example/report.pdf"}
{"status":"error","code":"asset_hash_mismatch","detail":"report.zip"}
```

### Asset manifest

| File in `--assets` | SHA-256 |
| --- | --- |
| `report.zip` | `0578b55567ccb977a9857a7a93287a6fd130abf64ef533e61b00ab63b82ea7b3` |
| `NanumGothic-Regular.ttf` | `76f45ef4a6bcff344c837c95a7dcc26e017e38b5846d5ae0cdcb5b86be2e2d31` |

The font's hash equals that of `apps/academy/dc-gang-a/src/academy/web/assets/NanumGothic-Regular.ttf`
in this repository. On the host the directory is `/usr/local/lib/academy-mcp/studyq-assets/`,
read-only (R11).

### `course_book` templates and defects

| Page | Template |
| --- | --- |
| 1 to 6 | `coursebook_1.jpg` to `coursebook_6.jpg`, one page each |
| 7 and after | `coursebook_7.jpg`, repeated as its rows need |

The three defects of `StudyQReport.exe` that the executor does not reproduce: the swapped
columns of page 4 (R14), the chart drawn 36 px from its box (R15), and the raw `rn` tokens
left where the input breaks a line (R16).

### Line-break tokens

A line-break token is `\r\n` or `\n` in a string of the input, or one of their escaped forms
`\\r\\n` and `\\n`. A raw `rn` token is the two characters `rn` printed at the place of such a
token (the token with its backslashes lost); an `rn` inside a word of the input is not one.

### The parity check

The expected field table lists, for each field of a type, its page and its box on the
template, as the decompiled drawing calls place it, with the corrections of R14 and R15. The
check renders the synthetic fixture, extracts the text of every page with its position, and
counts a mismatch for each field whose text is absent from its page or lies outside its box
by more than 2 pt, and for each text on a page that no field explains. It also counts the raw
`rn` tokens (R16).

### cism templates

| Group | Templates | `k` (filled `block%d` fields of the group) |
| --- | --- | --- |
| 1 to 3 | `cism_1.jpg`, `cism_2.jpg`, `cism_3.jpg` | none |
| 4 | `4_1.jpg`, `4_2.jpg` | 1 or 2 |
| 5 | `5_1.jpg` to `5_4.jpg` | 1 to 4 |
| 6 | `6_1.jpg`, `6_2.jpg` | 1 or 2 |

### Print archive path templates

| `kind` | Path (printf form) |
| --- | --- |
| `exam` | `/data/contents/exam/d6-%03d/d4-%02d/d2-%02d/ek-%u/%u-print.zip` |
| `examtext` | `/data/contents/examtext/d4-%02d/d2-%02d/etk-%u/%u-print.zip` |

A `clinic` input names its archives through `clinic_list`, `exam_key`, `content_list`,
`clinic_type` and `lecture_value`, the lists delimited by `|,|`; `clinic_list` and
`content_list` are paired element by element. The archive key is host configuration (R23):
no key, and no file that holds one, is in this repository.

### Error codes

| Code | When |
| --- | --- |
| `usage_invalid` | R3 |
| `input_invalid` | R4, R18, R21, R22 |
| `type_unknown` | R5 |
| `output_exists` | R6 |
| `asset_missing`, `asset_hash_mismatch` | R9 |
| `archive_key_missing` | R23 |
| `archive_missing` | an archive of R20 that does not exist |
| `archive_refused` | R24, R25 |
| `render_failed` | any other failure, after which R2 holds |

### Limits

| Limit | Value |
| --- | --- |
| Standard input | 4 MiB |
| `detail` | 256 bytes |
| Members of an archive | 4096 |
| One member, decompressed | 64 MiB |
| One archive, decompressed | 256 MiB |

### Tests

`modules/academy-workbench/tools/studyq-linux/tests/test_core.py`, with synthetic fixtures of
invented values only. The tests that render need PyMuPDF (R27); the parity tests against the
official templates need the assets directory, which CI does not have.

## Not included

- The six other official report types of `StudyQReport.exe`; each is added by a later
  revision of this specification with its own rules.
- How the TypeScript server fetches the input from the LMS and where it keeps the PDF; that
  is [tool-surface.md](../academy-mcp/tool-surface.md) and the tools' own contracts.
- Pixel equality with the PDFs of the Windows programs: only the field placement of the
  parity check is promised.
- Installing the assets or the archive key on a host.
