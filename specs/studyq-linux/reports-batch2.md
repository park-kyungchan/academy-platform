# The Gang-A, NA score and SmartBook reports: `render --type ganga|na_score|smart_book`

## Purpose

`StudyQReport.exe` makes three more official report PDFs for a teacher: the Gang-A report
(`ganga`), the NA score report (`na_score`) and the SmartBook report (`smart_book`). This contract
adds them to the Linux executor `python3 -m studyq_linux` ([core.md](core.md)), so that all eight
report types of `StudyQReport.exe` run on Linux: one JSON object on standard input, one PDF on the
official templates of the pinned asset manifest, one line of answer. Every rule of core.md (the
form and answer, the assets, the error codes, the limits, R12 and R16) holds for the three types;
this contract states what is particular to them. It is relied on by the TypeScript Academy
server's tool `academy_official_report_pdf_get` (`reportType` `ganga`, `na_score`, `smart_book`,
[tool-surface.md](../academy-mcp/tool-surface.md)), which reads the input from the LMS's read-only
`ReportViewServlet` views `GaReportView`, `NaScoreResultView` and `SmartBookReportView`
([direct-key-lookup.md](../academy-mcp/direct-key-lookup.md) R8), and by every later change of the
three renderers. The template sequences and overlay names were read from the decompilation of
`StudyQReport.exe`; that decompilation is not part of this repository.

## Requirements

- R1: `render --type <type>` for `<type>` `ganga`, `na_score` or `smart_book` answers as core.md R1, with `reportType` the type given, and draws its pages on the templates of the table "Template sequences" in that order: for `ganga`, `image/ganga/ganga_1.jpg` to `ganga_6.jpg` and one more page on `ganga_6.jpg` for each further 25 questions; for `na_score`, `image/na_score/score_1.jpg` to `score_3.jpg` and one more page on `score_3.jpg` for each further 25 questions; for `smart_book`, `image/smart_book/smartbook_1.jpg` to `smartbook_3.jpg`, for each chapter `smartbook_4.jpg` and one more page on `smartbook_5.jpg` for each further 25 of its questions, then `smartbook_6.jpg` and `smartbook_7.jpg`. A synthetic input of 20 questions gives 6 and 3 pages, and one of 3 chapters of at most 25 questions 8 pages. The input of one type is refused by the others, and by the five types of core.md R5, with `input_invalid`. Tests: test_r1_template_sequences, test_r1_rendered_page_counts_follow_the_template_sequence, test_r1_types_are_known_and_refuse_the_other_types_inputs, test_r5_type_unknown.
- R2: A plan reads only the templates of R1, the overlays of the table "Overlays", each named by the input (`pyramid<pyramid_tier>`, `signal_0<rank_signal>`, `0<rank_band>`, `lv0<grade>`), and `image/mark_o.jpg`, `image/mark_x.jpg`; no asset of another report type. An overlay that the manifest does not hold refuses with `asset_missing` and writes no PDF. Tests: test_r2_required_assets_are_the_official_overlays, test_r2_missing_overlay_refuses_with_asset_missing.
- R3: The pages are A4, `595.2756 x 841.8898 pt`, 1 template pixel is 0.24 pt and `y_fitz = 841.8898 - y_hpdf`. Each value is printed in its own box of the table "Fields", strictly inside the page, and no two boxes of a page overlap; a value that does not fit its box is shrunk down to 6 pt and then cut with `...`; the comment is wrapped into its box and its last line cut with `...` when the box is full. The unit tables hold exactly one row for each unit of the input and no other row; a question count of a unit is the number of questions of the input whose `unit` is its name (0 when none), and no other count is made. `smart_book` is the layout of `course_book` (core.md R13-R17, R29) with every template and overlay read from `image/smart_book/`: the same fields, boxes, shapes and picture places. Tests: test_r3_fields_fit_their_boxes_inside_a4_without_overlap, test_r3_long_values_shrink_to_6pt_then_are_cut, test_r3_comment_is_wrapped_and_cut_with_an_ellipsis, test_r3_question_rows_columns_and_marks, test_r3_unit_counts_come_from_the_questions_only, test_r3_scores_and_pictures_follow_the_input, test_r3_smart_book_is_the_course_book_layout_on_its_own_templates, test_batch1_layouts_are_unchanged_by_the_shared_pieces.
- R4: The `ganga` input is one JSON object with exactly the members of the table "`ganga` input" and their limits; an unknown or missing member, a member of another JSON type, or a value out of its limit refuses with `input_invalid`. Tests: test_r4_ganga_members_are_strict.
- R5: The `na_score` input is one JSON object with exactly the members of the table "`na_score` input" and their limits, under the same refusals as R4. Tests: test_r5_na_score_members_are_strict.
- R6: The `smart_book` input holds exactly the eleven members of the `course_book` input (core.md R4, under its limits) and `book_code`, a string of 1 to 64 characters of `A-Z`, `a-z`, `0-9`, `.`, `_` and `-` (validated and not printed: it is the key of the LMS read), under the same refusals as R4. Tests: test_r6_smart_book_adds_book_code_to_the_course_book_members.
- R7: The `detail` of a refusal of R4 to R6 names the member path only (`units[1].name`, `questions[2].no`, `book_code`), never a value or an unknown key of the input (core.md R7). Tests: test_r7_details_hold_no_input_value.
- R8: The same input with the same assets gives the same PDF bytes: the PDF is saved with no new identifier and cleared metadata, and holds no creation or modification time; its only font is NanumGothic (core.md R8, R12). Tests: test_r8_the_plan_is_deterministic, test_r8_same_input_same_bytes_and_no_time_or_id.
- R9: A line-break token of a string of the input (core.md, "Line-break tokens") breaks the comment's line and becomes one space in a one-line value; it is never printed, so no raw `rn` token is planned; an empty string is no field, so the parity check never searches for ''. Tests: test_r9_line_break_tokens_break_and_no_raw_rn_is_planned, test_r3_fields_fit_their_boxes_inside_a4_without_overlap.
- R10: The parity check (`python3 -m studyq_linux.parity --type ganga|na_score|smart_book --pdf <pdf> --assets <dir> < input.json`, core.md "The parity check") reports 0 mismatches and 0 raw `rn` tokens on the synthetic fixtures over synthetic templates and over the official templates of `STUDYQ_ASSETS_DIR`, and counts a displaced field and a raw `rn`. Tests: test_r10_parity_reports_zero_mismatches_and_zero_raw_rn, test_r10_parity_counts_a_displaced_field_and_a_raw_rn, test_r10_parity_against_the_official_templates, test_r10_parity_module_takes_the_new_types.

## Interface

### Forms

```
python3 -m studyq_linux render --type ganga|na_score|smart_book --assets <dir> --out <pdf>   < input.json
python3 -m studyq_linux.parity --type ganga|na_score|smart_book --pdf <pdf> --assets <dir>   < input.json
```

### Template sequences

| Type | Pages |
| --- | --- |
| `ganga` | `image/ganga/ganga_1.jpg` cover, `ganga_2.jpg` results, `ganga_3.jpg` test information, `ganga_4.jpg` achievement, `ganga_5.jpg` (no field of the input), `ganga_6.jpg` questions 1 to 25 (15 in the left table, 10 in the right), then `ganga_6.jpg` for each further 25 |
| `na_score` | `image/na_score/score_1.jpg` cover, `score_2.jpg` results and unit table, `score_3.jpg` questions 1 to 25, then `score_3.jpg` for each further 25 |
| `smart_book` | `image/smart_book/smartbook_1.jpg` cover, `smartbook_2.jpg` grade and averages, `smartbook_3.jpg` tests, for each chapter `smartbook_4.jpg` (its questions 1 to 25) and `smartbook_5.jpg` for each further 25, then `smartbook_6.jpg` difficulty and `smartbook_7.jpg` units |

### Overlays

The folders of the `ganga` overlays count the pages from 0 (`1p` is page 2).

| Type | Overlay | Where |
| --- | --- | --- |
| `ganga` | `image/ganga/1p/01 pyramid<pyramid_tier>.jpg` | results page, inside `(212, 1250, 2269, 2450)` px, never enlarged |
| `ganga` | `image/ganga/3p/03 graph.jpg`, `03 average-point.png`, `03 oneself-point.png` | achievement page at `(236, 520)` px; the two points on the axis of each of the first four units (top, right, bottom, left) at `483 px x value / 100` from the centre `(805, 632)` of the graph |
| `ganga` | `image/ganga/5p/05 signal_0<rank_signal>.jpg` | first question page, at `(1798, 140)` px |
| `na_score` | `image/na_score/0<rank_band>.jpg` | results page, inside `(212, 400, 1200, 1200)` px, never enlarged |
| `smart_book` | `image/smart_book/1p/lv0<grade>.jpg` | page 2, where `course_book` draws `1p/grade_<grade>.jpg` |
| `smart_book` | `image/smart_book/5p/score_range_chart.jpg`, `5p/test_list_chart.jpg`, `5p/my_score_point.png`, `5p/avg_score_point.png` | page 3, as the charts and markers of `course_book` |
| `smart_book` | `image/smart_book/4p/munit_chart.jpg`, `4p/my_score_point.png` | page 7, as the unit chart of `course_book` |
| all | `image/mark_o.jpg`, `image/mark_x.jpg` | the result cell of each question row, by `correct` |

### Fields

Boxes in template pixels `(x0, y0, x1, y1)`; sizes in pt. `p<n>` is the page number. The fields of
`smart_book` are those of `course_book` (core.md R13-R17).

| Field | Box | Align, size |
| --- | --- | --- |
| `p1.title` | `(779, 1286, 2258, 1400)` | left, 18 |
| `p1.test_date`, `p1.student` | `(1732, 3048, 2345, 3136)`, `(1732, 3141, 2345, 3229)` | left, 14 |
| `p2.my_score`, `p2.avg_score` | `(1330, 639, 1654, 744)`, `(1330, 1033, 1654, 1138)` | right, 24 |
| `p2.comment.<k>` (`ganga`) | lines of 17.6 pt in `(945, 2538, 2240, 3238)` | left, 11 |
| `p3.questions`, `p3.<level>.questions`, `p3.u<i>.questions` (`ganga`) | as `formative` (reports-batch1.md "Fields") | right 20, right 16, centre 20 |
| `p4.u<i>.name`, `.my_score`, `.average`, `p4.<level>.score` (`ganga`) | as `formative` | at most 9; centre 14 |
| `p<n>.q<i>.no`, `.difficulty`, `.correct_rate` (`ganga`) | as `formative` on `ganga_6.jpg`; `unit` has no column and is not printed | centre, 10.5 |
| bars (`na_score`) | the score and the average from 309.5 px, 18.545 px a point, rows `(1320, 1382)` and `(1452, 1514)` | |
| `p2.u<i>.name`, `.my_score`, `.average` (`na_score`) | one row a unit in `(212, 1600, 2269, 2464)`, at most 98 px a row; columns 212-1420, 1420-1845, 1845-2269 px, 20 px inside | name left, scores centre; at most 11 |
| `p2.comment.<k>` (`na_score`) | lines of 17.6 pt in `(245, 2560, 2240, 3260)` | left, 11 |
| `p<n>.q<i>.no`, `.unit`, `.difficulty`, `.correct_rate` (`na_score`) | as the `diag` question table with `unit` in the area column: rows from 334 px, 98 px a row | centre (unit left), 10.5 |

`ganga` prints no rank number: the rank is the signal picture. `book_code` of `smart_book` is not
printed.

### `ganga` input

One JSON object with exactly these members (an example of the shapes is in the tests, with
invented values):

| Member | Value |
| --- | --- |
| `title`, `student`, `test_date` | strings of 1 to 120, 1 to 64 and 1 to 32 characters |
| `pyramid_tier` | an integer from 1 to 3 |
| `my_score`, `avg_score` | numbers from 0 to 100 |
| `rank_signal` | an integer from 1 to 5 |
| `comment` | a string of 0 to 3000 characters with line-break tokens; may be empty |
| `units` | 1 to 12 objects `{name, my_score, average}`: `name` 1 to 80 characters, the scores 0 to 100 |
| `difficulty` | an object of `top`, `high`, `mid`, `low`, each `{questions, score}`: `questions` an integer from 0 to 500, `score` 0 to 100 |
| `questions` | 1 to 100 objects `{no, unit, difficulty, correct_rate, correct}`: `no` 1 to 500, `unit` 1 to 80 and `difficulty` 1 to 32 characters, `correct_rate` 0 to 100, `correct` a boolean |

### `na_score` input

| Member | Value |
| --- | --- |
| `title`, `student`, `test_date`, `my_score`, `avg_score`, `comment`, `units`, `questions` | as `ganga` |
| `rank_band` | an integer from 1 to 4 |

### `smart_book` input

The eleven members of the `course_book` input (core.md "`course_book` input") and `book_code`
(`^[A-Za-z0-9._-]{1,64}$`).

### Tests

`modules/studyq-linux/tests/test_reports_batch2.py` (and `test_r5_type_unknown` of `test_core.py`),
with synthetic fixtures of invented values only. The pure-Python tests run in `./check`
unconditionally; the rendering tests run under core.md R27, and the parity test against the
official templates only with `STUDYQ_ASSETS_DIR`.

## Not included

- How the TypeScript server reads the input from the LMS; that is
  [direct-key-lookup.md](../academy-mcp/direct-key-lookup.md) R8 and the tool's own contract.
- Units beyond the first four on the `ganga` graph: the graph has four axes; every unit is in its
  list.
- The page `ganga_5.jpg`: the input holds no data for it, and nothing is invented.
- Pixel equality with the PDFs of `StudyQReport.exe`: only the field placement of the parity check
  is promised.
