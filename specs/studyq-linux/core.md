# The Linux StudyQ print executor: `python3 -m studyq_linux`

## Purpose

The Academy's print PDFs of existing exam and clinic papers are made today by Windows programs
(`CreateTotalPrint`, `CreateClinic`). `studyq_linux` makes the same PDFs on Linux from the
papers' `print.zip` archives, as one JSON document names them. It is one-shot: one run, one PDF,
one line of answer. It is relied on by the TypeScript Academy server's tool
`academy_paper_print_pdf_get` ([tool-surface.md](../academy-mcp/tool-surface.md)), by the units
that build and check the executor, and by every later change of it. The official report PDFs of
`academy_official_report_pdf_get` are not made here: they are made by the original
`StudyQReport.exe` on the user's PC ([pc-executor.md](../academy-mcp/pc-executor.md)), and the
report renderers of this module were removed. The rules were read from the decompilation of the
Windows programs; that decompilation is not part of this repository.

## Requirements

Form and answer

- R1: `assemble-print --out <pdf>` reads one UTF-8 JSON object from standard input and, when it succeeds, writes the PDF at `<pdf>`, ends with exit status 0 and prints exactly one line on standard output: a JSON object with exactly the members `status` (`"ok"`), `reportType` (`print_<kind>`), `pages` (the number of pages of the PDF, at least 1), `sha256` (64 lowercase hex digits, the SHA-256 of the bytes at `outputPath`) and `outputPath` (the absolute path of `<pdf>`). Tests: test_r20_exam_archives_become_one_pdf_in_input_order, test_r20_examtext_and_clinic_kinds_answer_their_report_type.
- R2: A run that fails ends with a non-zero exit status and prints exactly one line on standard output, `{"status":"error","code":<code>,"detail":<text>}`, `<code>` one of the table "Error codes"; no file is left at `<pdf>`, and a file that was there before the run is unchanged. Tests: test_r2_error_line_is_one_line_with_a_known_code, test_r2_a_refused_run_leaves_no_file_and_an_existing_output_is_kept.
- R3: The words are the form of the Interface: an unknown verb (`render` included, which was removed), a missing, repeated or unknown flag, or a positional word refuses with `usage_invalid`, before standard input is read. Tests: test_r3_usage_invalid_before_stdin_is_read, test_entry_point_is_python_dash_m.
- R4: Standard input that is not valid UTF-8, larger than 4 MiB, not exactly one JSON object, or that lacks a member the kind requires, holds a member the kind does not take, or holds a member of another JSON type, refuses with `input_invalid`. Tests: test_r4_input_invalid.
- R5: (removed)
- R6: An existing `<pdf>` refuses with `output_exists`; the PDF is written beside `<pdf>` under another name and linked to `<pdf>` only when complete, so `<pdf>` is never seen partly written, and the temporary file is removed when the link loses a race. Tests: test_r6_written_beside_and_renamed_never_partial, test_r6_temp_file_removed_when_the_rename_loses_a_race, test_r20_an_existing_output_is_refused_before_anything_is_read.
- R7: `detail` and standard error hold no value of the input (no student name, record text, student id or key) and no archive key; `detail` is at most 256 bytes. Tests: test_r7_details_hold_no_input_value, test_r7_unexpected_failure_reports_class_only, test_r7_refusal_details_hold_no_input_value_and_no_path.
- R8: The same input gives the same PDF bytes, so the same `sha256`: the PDF holds no creation or modification time and no random identifier. Tests: test_r8_the_same_input_gives_the_same_bytes_with_no_time_or_id.

Assets

- R9: The asset manifest check reads `manifest.json` in an assets directory `<dir>` (Interface, "Asset manifest"; the pinned templates and font that stay installed on the host for the units that read them) and checks it: a manifest that is absent, not one JSON object of exactly the five members of the Interface, of a `schemaVersion` other than `studyq-assets/1`, with a `fileCount` that is not the number of `files`, with no `files` entry for the font, or with a `files` path that is absolute, holds a `..` component or lies outside `image/` and the font refuses with `asset_manifest_invalid`; a `reportZipSha256` or `fontSha256` other than the pinned value, a `files` entry for the font other than `fontSha256`, or a file whose SHA-256 differs from its entry under `files` (checked again when the file is read) refuses with `asset_hash_mismatch`; a file that `files` names and the directory lacks, or that is not a regular file, refuses with `asset_missing`. Tests: test_r9_good_manifest_gives_verified_bundle, test_r9_manifest_absent_or_malformed, test_r9_schema_members_and_consistency, test_r9_unsafe_paths_refused, test_r9_pins_and_hash_mismatch, test_r9_missing_file_and_missing_font_entry, test_r9_symlink_is_not_followed, test_r9_file_changed_after_verification_is_refused_on_read, test_r9_repo_font_matches_the_pins.
- R10: The executor never creates, changes or removes a file under an assets directory, and opens no network connection. Tests: test_r10_assets_are_untouched_and_the_network_is_not_used.
- R11: `report.zip`, the templates inside it and the print archives are never committed to Git: no file of the repository has the SHA-256 of `report.zip` or of a template inside it. On the host they are installed read-only under `/usr/local/lib/academy-mcp/studyq-assets/`. (no test yet; observed by the unit that installs studyq-linux)
- R12: (removed)

The report types

- R13: (removed)
- R14: (removed)
- R15: (removed)
- R16: (removed)
- R17: (removed)
- R18: (removed)
- R19: (removed)

`print.zip`

- R20: `assemble-print --out <pdf>` reads one UTF-8 JSON object (R4) from standard input naming `archive_root`, `kind` (`exam`, `examtext` or `clinic`) and the keys of that kind (Interface, "Print archive input"), validates all of it before it opens any file, and then opens only the local archives that the path templates of the Interface give under `archive_root`; it opens no network connection, so R10 holds (the TypeScript server downloads the archives and passes only their directory). It writes one PDF with the pages of the archives in the order of the input's list and, within an archive, in the order of its member list, and answers as R1 with `reportType` `print_<kind>`. Of each archive the members used are the PNG members `print-<sel_type>-exam.png`, `print-obj-objective.png` (only when `sel_type` is `obj`), `print-<sel_type>-answer.png` (only with `include_answers`) and `print-<sel_type>-explain.png` (only with `include_explain`), or its `.pdf` members; an archive that holds both kinds of member, or neither, or a PNG member that does not begin with the PNG signature or a PDF member that does not begin with `%PDF-`, refuses with `archive_refused`. The PNG members are stacked on A4 pages (`595.2756 x 841.8898 pt`) from a 36 pt margin at the width `523.2756 pt`, keeping the aspect ratio; an image that would end below `805.8898 pt` starts a new page and one taller than `769.89 pt` is scaled to that height; the pages of a PDF member are appended as they are. The PDF holds no time, identifier or metadata (R8). Tests: test_r20_exam_and_examtext_input_schema_is_strict, test_r20_a_good_input_is_normalised_with_defaults, test_r20_an_existing_output_is_refused_before_anything_is_read, test_r20_members_are_chosen_by_the_selection_and_kept_in_member_order, test_r20_archives_are_read_in_the_order_of_the_input_list, test_r20_pdf_members_are_taken_whole_and_never_mixed_with_png_members, test_r20_a_missing_archive_refuses_before_any_archive_is_read, test_r20_an_archive_that_is_a_link_or_corrupt_is_refused, test_r8_images_stack_on_a4_pages_at_the_fitting_width, test_r8_a_taller_image_is_scaled_to_the_page_and_fills_it, test_r8_a_wide_image_keeps_its_aspect_ratio, test_r20_exam_archives_become_one_pdf_in_input_order, test_r20_a_page_overflow_starts_a_new_page, test_r8_the_same_input_gives_the_same_bytes_with_no_time_or_id, test_r20_examtext_and_clinic_kinds_answer_their_report_type, test_r20_pdf_members_are_appended_page_by_page, test_r2_a_refused_run_leaves_no_file_and_an_existing_output_is_kept, test_r20_a_member_that_pymupdf_cannot_draw_refuses.
- R21: An archive path is made only from the templates of the Interface with integer keys: a `lecture_key` or `exam_key` that is not an integer from 0 to 999999999, an `examtext_key` that is not an integer from 0 to 999999, or a `d6`, `d4` or `d2` value outside the range its width allows refuses with `input_invalid`, and no path is made from any other input text. Tests: test_r21_path_templates_follow_the_digit_splits, test_r21_keys_out_of_range_refuse_input_invalid_and_name_the_member_only.
- R22: A `clinic` input holds exactly `archive_root`, `kind`, `clinic_list`, `exam_key`, `content_list`, `clinic_type` and `lecture_value`, and optionally `sel_types` or `sel_type` (not both), `include_answers` and `include_explain`; the list members are strings of elements joined by `|,|`, and an empty element, or lists of different element counts where the Interface pairs them, refuses with `input_invalid`. Tests: test_r22_clinic_input_resolves_each_exam_key_through_the_exam_template, test_r22_clinic_input_refuses_a_bad_member_and_names_its_path_only, test_r22_two_hundred_clinic_items_are_accepted.
- R23: The archive key is read from the private host file that the environment variable `STUDYQ_PRINT_KEY_FILE` names (a regular file, not a symbolic link, of the caller, with no group or other permission bit, of 1 to 256 bytes; one trailing line break is not part of the key); it is never read from standard input, an argument or the repository, and is never printed or written. A missing variable or file, or a file of another kind, owner, mode or size, refuses with `archive_key_missing` before any archive is opened. Tests: test_r23_a_missing_variable_or_file_refuses_before_any_archive_is_opened, test_r23_a_file_of_another_mode_owner_or_size_refuses, test_r23_a_qualified_file_gives_the_key_without_its_trailing_line_break_only, test_r23_a_wrong_key_refuses_and_no_key_is_printed.
- R24: An archive member whose name is absolute, holds a `..` component or a drive letter, resolves outside the extraction directory, or is a symbolic link refuses the whole run with `archive_refused` before any member is read. Tests: test_r24_unsafe_member_names_refuse_the_whole_run_before_any_member_is_read.
- R25: An archive with more than 4096 members, a member that decompresses to more than 64 MiB, or members that decompress to more than 256 MiB together refuses with `archive_refused`; the limits count the bytes actually decompressed, not the sizes the archive declares, and decompression stops at the first byte over a limit. A corrupt archive or a wrong key also refuses with `archive_refused`. Tests: test_r25_more_than_4096_members_refuse_before_any_member_is_read, test_r25_a_member_over_the_limit_refuses_whatever_the_archive_declares, test_r25_members_over_the_archive_limit_together_refuse, test_r25_the_documented_limits_are_the_constants, test_r25_a_member_of_64_mib_plus_one_byte_refuses, test_r7_refusal_details_hold_no_input_value_and_no_path.

Checks

- R26: The pure-Python tests (words, input, asset manifest, path templates, ZipSlip, size limits, image headers) run in `./check` unconditionally, with no asset and no PyMuPDF. Tests: test_r26_pure_tests_do_not_import_pymupdf_for_planning, test_r26_image_dimensions_are_read_from_headers.
- R27: The tests that draw a PDF run when `CHECK_STUDYQ_PYMUPDF=1` is set or the module `pymupdf` can be imported, and otherwise print one skip line and pass; drawing without PyMuPDF refuses with `render_failed`. Tests: test_r27_rendering_without_pymupdf_refuses_with_render_failed, test_r27_gate_condition_is_documented_in_the_module.
- R28: `./check` fails when a file under `specs/` or `modules/studyq-linux/` (a synthetic fixture of the executor included) holds a secret value: a private key block, a token of a known form, a bearer credential, or a quoted value assigned to a password, secret, key or token name (privacy check I6). When `CHECK_STUDYQ_ROSTER` names a file of student names, one per line, it also fails when one of them appears. It prints file and line only, never the text it matched, and does not read `imported/`, `private/` or `private-students/` trees. Tests: test_r28_check_passes_on_the_current_specs_and_module, test_r28_check_fails_on_secret_values, test_r28_check_passes_on_plain_text_and_the_two_pinned_hashes_and_skips_private_trees, test_r28_roster_names_fail_the_check_without_being_printed, test_r28_check_wiring.
- R29: (removed)
- R30: The report renderers are retired: the module holds no report renderer, layout, parity check or report-input validator, and the rules of the report types are marked removed here. Tests: test_retired_report_rules_are_marked_in_the_specification.

## Interface

### Form

```
python3 -m studyq_linux assemble-print --out <pdf>   < input.json
```

An example answer, with invented values:

```json
{"status":"ok","reportType":"print_exam","pages":2,"sha256":"<64 hex>","outputPath":"/tmp/example/print.pdf"}
{"status":"error","code":"archive_refused","detail":"png and pdf members"}
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

### Print archive path templates

| `kind` | Path under `archive_root` (printf form) |
| --- | --- |
| `exam` | `data/contents/exam/d6-%03d/d4-%02d/d2-%02d/ek-%u/%u-print.zip` |
| `examtext` | `data/contents/examtext/d4-%02d/d2-%02d/etk-%u/%u-print.zip` |

For `exam` the key `k` is an integer from 0 to 999999999 and `d6 = k // 1000000` (0 to 999),
`d4 = (k % 1000000) // 10000` (0 to 99), `d2 = (k % 10000) // 100` (0 to 99); for `examtext` the key
is an integer from 0 to 999999, `d4 = k // 10000` and `d2 = (k % 10000) // 100`; `%u` is the key
itself (R21). A `clinic` input resolves each of its `exam_key` elements through the `exam` path.

### Print archive input

`archive_root` is the same member of all three kinds: an absolute path of an existing directory
that is not a symbolic link (its real path is itself); every archive is read under it and
nothing else is opened. The input is one JSON object with exactly these members:

| `kind` | Members |
| --- | --- |
| `exam` | `archive_root`, `kind`, `items`: 1 to 200 objects `{lecture_key, sel_type, include_answers?, include_explain?}` |
| `examtext` | `archive_root`, `kind`, `items`: 1 to 200 objects `{examtext_key, sel_type, include_answers?, include_explain?}` |
| `clinic` | `archive_root`, `kind`, `clinic_list`, `exam_key`, `content_list`, `clinic_type`, `lecture_value`, and optionally `sel_types` or `sel_type`, `include_answers`, `include_explain` |

`sel_type` is `obj`, `sbj` or `des`; `include_answers` and `include_explain` are booleans that
default to `false`. In a `clinic` input the lists `clinic_list`, `exam_key`, `content_list` and
`lecture_value` are strings of 1 to 200 non-empty elements joined by `|,|` and have the same
number of elements, paired element by element (the four-way list pairing); each `exam_key`
element is an integer from 0 to 999999999 in decimal digits; `clinic_type` is a non-empty
string; `sel_types` is a `|,|` list of one `sel_type` for each element, or `sel_type` gives one
for all of them (`obj` when neither is given; both given refuses). The members of an
archive are ZipCrypto-encrypted PNG files, or PDF files, under the archive key (R23): no key,
and no file that holds one, is in this repository, and a test makes its own at run time.

### Error codes

| Code | When |
| --- | --- |
| `usage_invalid` | R3 |
| `input_invalid` | R4, R20, R21, R22; the detail names a member path of the input, never a value |
| `output_exists` | R6 |
| `asset_manifest_invalid`, `asset_missing`, `asset_hash_mismatch` | R9 |
| `archive_key_missing` | R23 |
| `archive_missing` | an archive of R20 that does not exist |
| `archive_refused` | R20 (member selection), R24, R25 |
| `not_implemented` | reserved for future unbuilt targets; no verb answers it now; no file is written |
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

`modules/studyq-linux/tests/test_core.py` and `test_print_zip.py`, with synthetic fixtures of
invented values only. The tests that draw a PDF need PyMuPDF (R27). Python's `zipfile` cannot
write encrypted archives and no `zip` or `7z` program is assumed, so `test_print_zip.py` holds a
small ZipCrypto writer and makes its passphrase at run time in a scratch file of mode 0600.

## Not included

- The official report PDFs (`course_book`, `cism`, `diag`, `new_diag`, `formative`, `ganga`,
  `na_score`, `smart_book`): [pc-executor.md](../academy-mcp/pc-executor.md).
- How the TypeScript server fetches the archives and where it keeps the PDF; that is
  [tool-surface.md](../academy-mcp/tool-surface.md) and the tools' own contracts.
- Installing the assets or the archive key on a host.
