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

- R1: `render --type <type> --assets <dir> --out <pdf>` reads one UTF-8 JSON object from standard input and, when it succeeds, writes the PDF at `<pdf>`, ends with exit status 0 and prints exactly one line on standard output: a JSON object with exactly the members `status` (`"ok"`), `reportType` (the `<type>` given), `pages` (the number of pages of the PDF, at least 1), `sha256` (64 lowercase hex digits, the SHA-256 of the bytes at `outputPath`) and `outputPath` (the absolute path of `<pdf>`). Tests: test_r1_success_line_and_pdf, test_r1_exactly_one_line_on_stdout_through_python_dash_m.
- R2: A run that fails ends with a non-zero exit status and prints exactly one line on standard output, `{"status":"error","code":<code>,"detail":<text>}`, `<code>` one of the table "Error codes"; no file is left at `<pdf>`, and a file that was there before the run is unchanged. Tests: test_r2_error_line_is_one_line_with_a_known_code, test_r2_failed_run_leaves_no_file_and_keeps_an_existing_one.
- R3: The words are one of the two forms of the Interface: an unknown verb, a missing, repeated or unknown flag, or a positional word refuses with `usage_invalid`, before standard input is read. Tests: test_r3_usage_invalid_before_stdin_is_read, test_entry_point_is_python_dash_m.
- R4: Standard input that is not valid UTF-8, larger than 4 MiB, not exactly one JSON object, or that lacks a member the type requires, holds a member the type does not take, or holds a member of another JSON type, refuses with `input_invalid`. Tests: test_r4_input_invalid.
- R5: `<type>` of `render` is `course_book` or `cism`; any other value refuses with `type_unknown`. Tests: test_r5_type_unknown, test_interim_unbuilt_targets_refuse_not_implemented.
- R6: An existing `<pdf>` refuses with `output_exists`; the PDF is written beside `<pdf>` under another name and renamed to `<pdf>` only when complete, so `<pdf>` is never seen partly written. Tests: test_r6_written_beside_and_renamed_never_partial, test_r6_temp_file_removed_when_the_rename_loses_a_race.
- R7: `detail` and standard error hold no value of the input (no student name, record text, student id or key) and no archive key; `detail` is at most 256 bytes. Tests: test_r7_details_hold_no_input_value, test_r7_unexpected_failure_reports_class_only.
- R8: The same input with the same assets gives the same PDF bytes, so the same `sha256`: the PDF holds no creation or modification time and no random identifier. Tests: test_r8_same_input_same_bytes_and_no_time_or_random_id, test_seam_renders_from_an_already_verified_bundle_without_a_directory.

Assets

- R9: Before it reads a template, `render` reads `manifest.json` in `<dir>` (Interface, "Asset manifest") and checks it: a manifest that is absent, not one JSON object of exactly the five members of the Interface, of a `schemaVersion` other than `studyq-assets/1`, with a `fileCount` that is not the number of `files`, with no `files` entry for the font, or with a `files` path that is absolute, holds a `..` component or lies outside `image/` and the font refuses with `asset_manifest_invalid`; a `reportZipSha256` or `fontSha256` other than the pinned value, a `files` entry for the font other than `fontSha256`, or a file whose SHA-256 differs from its entry under `files` (checked again when the file is read) refuses with `asset_hash_mismatch`; a file that `files` names and the directory lacks, or that is not a regular file, refuses with `asset_missing`. None of these writes a PDF. Tests: test_r9_good_manifest_gives_verified_bundle, test_r9_manifest_absent_or_malformed, test_r9_schema_members_and_consistency, test_r9_unsafe_paths_refused, test_r9_pins_and_hash_mismatch, test_r9_missing_file_and_missing_font_entry, test_r9_symlink_is_not_followed, test_r9_refusal_through_the_command_writes_no_pdf, test_r9_file_changed_after_verification_is_refused_on_read, test_r9_repo_font_matches_the_pins.
- R10: The executor never creates, changes or removes a file under `--assets`, and opens no network connection. Tests: test_r10_assets_are_untouched_and_the_network_is_not_used.
- R11: `report.zip`, the templates inside it and the print archives are never committed to Git: no file of the repository has the SHA-256 of `report.zip` or of a template inside it. On the host they are installed read-only under `/usr/local/lib/academy-mcp/studyq-assets/`. (no test yet; observed by the unit that installs studyq-linux)
- R12: The font of every page is `NanumGothic-Regular.ttf` of the manifest, embedded in the PDF; no other font is embedded. Tests: test_r12_only_nanum_gothic_is_embedded_on_every_page.

`course_book`

- R13: (removed)
- R14: On page 4 each value is printed in the column whose heading names its input member, not in the swapped columns of `StudyQReport.exe`. Tests: test_r14_page_4_values_sit_in_the_column_of_their_heading, test_r14_rows_start_at_1110_px_with_a_step_of_88_px_and_marks_follow_the_answer, test_r14_rendered_page_4_text_sits_in_its_own_columns.
- R15: The chart is drawn at the origin of its box on the template, without the 36 px offset of `StudyQReport.exe`. Tests: test_r15_charts_sit_at_the_origin_of_their_box_without_the_36_px_offset, test_r15_rendered_chart_is_placed_at_its_box_origin.
- R16: A line-break token of the input (Interface, "Line-break tokens") breaks the line and is not printed; no raw `rn` token appears in the text of any page. Tests: test_r16_line_break_tokens_break_and_are_not_printed, test_r16_no_raw_rn_token_in_any_page.
- R17: The parity check of the Interface, run on the synthetic full `course_book` fixture, reports 0 mismatches and 0 raw `rn` tokens. Tests: test_r17_parity_reports_zero_mismatches_and_zero_raw_rn, test_r17_parity_counts_a_displaced_field_and_an_unexplained_text, test_r17_parity_module_prints_counts_only, test_r17_parity_against_the_official_templates.

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

- R26: The pure-Python tests (words, input schema, asset manifest, path templates, ZipSlip, size limits, the parity of the field tables) run in `./check` unconditionally, with no asset and no PyMuPDF. Tests: test_r26_pure_tests_do_not_import_pymupdf_for_planning, test_r26_field_tables_are_deterministic_and_fit_their_boxes, test_r26_long_values_shrink_then_are_cut_with_an_ellipsis, test_r26_wrap_and_number_format, test_r26_image_dimensions_are_read_from_headers, test_r26_difficulty_page_shares_and_bars, test_r26_unit_chart_marks_sit_on_the_score_rows.
- R27: The rendering tests run when `CHECK_STUDYQ_PYMUPDF=1` is set or the module `pymupdf` can be imported, and otherwise print one skip line and pass; the parity check against the official templates runs only when an assets directory is given (`STUDYQ_ASSETS_DIR`), and otherwise prints one skip line. Tests: test_r27_rendering_without_pymupdf_refuses_with_render_failed, test_r27_gate_condition_is_documented_in_the_module, test_r27_gate_is_open_in_this_run.
- R28: `./check` fails when a file under `specs/` or `modules/studyq-linux/` (a synthetic fixture of the executor included) holds a secret value: a private key block, a token of a known form, a bearer credential, or a quoted value assigned to a password, secret, key or token name (privacy check I6). When `CHECK_STUDYQ_ROSTER` names a file of student names, one per line, it also fails when one of them appears. It prints file and line only, never the text it matched, and does not read `imported/`, `private/` or `private-students/` trees. Tests: test_r28_check_passes_on_the_current_specs_and_module, test_r28_check_fails_on_secret_values, test_r28_check_passes_on_plain_text_and_the_two_pinned_hashes_and_skips_private_trees, test_r28_roster_names_fail_the_check_without_being_printed, test_r28_check_wiring.
- R29: A `course_book` PDF is drawn on the extracted templates `image/course_book/coursebook_N.jpg` of the manifest in this order: page 1 on `coursebook_1.jpg`, page 2 on `coursebook_2.jpg`, page 3 on `coursebook_3.jpg`; then, for each chapter of `bunit_list` in the order of the input, one page on `coursebook_4.jpg` and, when the chapter has more than 25 questions, one page on `coursebook_5.jpg` for each further 25; then the penultimate page on `coursebook_6.jpg` and the final page on `coursebook_7.jpg`. A synthetic fixture of three chapters of at most 25 questions has exactly 8 pages. Tests: test_r29_template_sequence, test_r29_overflow_uses_template_5_and_page_count_follows_chapters, test_r29_rendered_pages_use_the_verified_template_sequence, test_r13_removed_rule_is_marked_in_the_specification.

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

`<dir>/manifest.json` is one JSON object with exactly these five members. It describes the
extracted layout installed under `<dir>`: `manifest.json`, `image/...` and `fonts/...`; no
`report.zip` file is required on disk.

| Member | Value |
| --- | --- |
| `schemaVersion` | `studyq-assets/1` |
| `reportZipSha256` | `0578b55567ccb977a9857a7a93287a6fd130abf64ef533e61b00ab63b82ea7b3`, the SHA-256 of `report.zip`: a provenance pin, checked by value only |
| `fontSha256` | `76f45ef4a6bcff344c837c95a7dcc26e017e38b5846d5ae0cdcb5b86be2e2d31`, the SHA-256 of `NanumGothic-Regular.ttf`; it equals the `files` entry of `fonts/NanumGothic-Regular.ttf` |
| `fileCount` | an integer equal to the number of entries of `files` |
| `files` | an object of per-file SHA-256 entries, `"<relative path>": "<64 lowercase hex>"`, one for each extracted file: the paths `image/...` and `fonts/NanumGothic-Regular.ttf`, relative to `<dir>`, never absolute and never with a `..` component |

An example of the shape, with the entries abbreviated: `{"schemaVersion":"studyq-assets/1","reportZipSha256":"0578b555…","fontSha256":"76f45ef4…","fileCount":2,"files":{"fonts/NanumGothic-Regular.ttf":"76f45ef4…","image/mark_o.jpg":"<64 hex>"}}`.

The font's hash equals that of `apps/academy/dc-gang-a/src/academy/web/assets/NanumGothic-Regular.ttf`
in this repository. On the host the directory is `/usr/local/lib/academy-mcp/studyq-assets/`,
read-only (R11).

### `course_book` input

One JSON object with exactly these members (an example of the shapes is in the tests, with
invented values):

| Member | Value |
| --- | --- |
| `period`, `student` | strings: the study period and the learner, printed on the cover |
| `grade` | an integer from 1 to 9: the grade picture `image/course_book/1p/grade_0<grade>.jpg` |
| `my_average`, `all_average` | numbers from 0 to 100 |
| `comment` | a string, with line-break tokens; may be empty |
| `score_range` | an object `{my, average}` of numbers from 0 to 100 |
| `test_list` | 1 to 8 objects `{no, title, questions, average, score, date}` |
| `bunit_list` | 1 to 30 chapters `{name, my_score, average, questions}`; `questions` is 1 to 200 objects `{no, type, question_type, difficulty, correct_rate, correct}` (`correct` is a boolean) |
| `difficulty` | an object of `top`, `high`, `mid`, `low`, each `{questions, score}` |
| `munit_list` | 1 to 11 objects `{name, correct, total, average}`, `correct` at most `total` |

A string that does not fit its box is shrunk to 6 pt and then cut with `...`; the comment is
wrapped and cut the same way at the end of its box.

### `course_book` templates and defects

| Page | Template |
| --- | --- |
| 1, 2, 3 | `image/course_book/coursebook_1.jpg`, `coursebook_2.jpg`, `coursebook_3.jpg` |
| each chapter | `coursebook_4.jpg`, then `coursebook_5.jpg` for each further 25 questions (R29) |
| penultimate | `coursebook_6.jpg` |
| final | `coursebook_7.jpg` |

The geometry is A4, `595.2756 x 841.8898 pt`, 3508 px tall at 300 dpi (1 px is 0.24 pt), and
`y_fitz = 841.8898 - y_hpdf`. Page 2 draws `1p/grade_0<grade>.jpg` at `(35.99, 745.89)`; page 3
draws `5p/score_range_chart.jpg` at `y = 721.89` and `5p/test_list_chart.jpg` at `y = 577.90`;
a chapter page puts its rows from 1110 px with a step of 88 px and `image/mark_o.jpg` or
`image/mark_x.jpg` at `x = 503.86`; the final page draws `4p/munit_chart.jpg` at `y = 697.90`.
A chart maps a score to the row `y(0) = 904.5` px, `y(100) = 144.5` px of its picture.

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
`rn` tokens (R16). It is run as

```
python3 -m studyq_linux.parity --pdf <pdf> --assets <dir>   < input.json
```

which prints one line of counts and field ids, never a value of the input, and ends with exit
status 0 only when both counts are 0. It is not a verb of the command (R3).

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
| `input_invalid` | R4, R18, R21, R22; the detail names a member path of the input, never a value |
| `type_unknown` | R5 |
| `output_exists` | R6 |
| `asset_manifest_invalid`, `asset_missing`, `asset_hash_mismatch` | R9 |
| `archive_key_missing` | R23 |
| `archive_missing` | an archive of R20 that does not exist |
| `archive_refused` | R24, R25 |
| `not_implemented` | `render --type cism` and `assemble-print`, until R18 to R25 are built; no file is written |
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

`modules/studyq-linux/tests/test_core.py`, with synthetic fixtures of invented values only. The
tests that render need PyMuPDF (R27); the parity tests against the official templates need the
assets directory, which CI does not have.

## Not included

- The six other official report types of `StudyQReport.exe`; each is added by a later
  revision of this specification with its own rules.
- How the TypeScript server fetches the input from the LMS and where it keeps the PDF; that
  is [tool-surface.md](../academy-mcp/tool-surface.md) and the tools' own contracts.
- Pixel equality with the PDFs of the Windows programs: only the field placement of the
  parity check is promised.
- Installing the assets or the archive key on a host.
