# The diagnostic, new-student diagnostic and formative reports: `render --type diag|new_diag|formative`

## Purpose

`StudyQReport.exe` makes three further official report PDFs for a teacher: the diagnostic report
(`diag`), the new-student diagnostic report (`new_diag`) and the formative-assessment report
(`formative`). This contract adds them to the Linux executor `python3 -m studyq_linux`
([core.md](core.md)): one JSON object on standard input, one PDF on the official templates of the
pinned asset manifest, one line of answer. Every rule of core.md (the form and answer, the
assets, the error codes, the limits, R12 and R16) holds for the three types; this contract states
what is particular to them. It is relied on by the TypeScript Academy server's tool
`academy_official_report_pdf_get` (`reportType` `diag`, `new_diag`, `formative`,
[tool-surface.md](../academy-mcp/tool-surface.md)), which reads the input from the LMS's read-only
`ReportViewServlet` views ([direct-key-lookup.md](../academy-mcp/direct-key-lookup.md) R8), and by
every later change of the three renderers. The page sequences were read from the decompilation of
`StudyQReport.exe` and checked against the official templates; that decompilation is not part of
this repository.

## Requirements

- R1: `render --type <type>` for `<type>` `diag`, `new_diag` or `formative` answers as core.md R1, with `reportType` the type given, and draws its pages on the templates of the table "Template sequences" in that order: for `diag`, `image/diag/diag_1.jpg` to `diag_9.jpg` and one more page on `diag_9.jpg` for each further 25 questions; for `new_diag`, `image/new_diag/diag_1.jpg` to `diag_10.jpg` (questions 1 to 25 on `diag_9.jpg`, the learning-style page on `diag_10.jpg`) and one more page on `diag_10.jpg` for each further 25 questions, its rows placed as on `diag_9.jpg`; for `formative`, `image/formative/formative_1.jpg` to `formative_6.jpg` and one more page on `formative_6.jpg` for each further 25 questions. A synthetic input of 20 questions gives 9, 10 and 6 pages. The input of one type is refused by the others with `input_invalid`. Tests: test_r1_template_sequences, test_r1_new_diag_questions_continue_on_template_10_after_the_learning_style_page, test_r1_rendered_page_counts_follow_the_template_sequence, test_r1_types_are_known_and_refuse_the_other_types_inputs, test_r5_type_unknown.
- R2: A plan reads only the templates of R1 and the overlays of the table "Overlays", each named by the input (`lv0<grade>`, `light_<signal>`, `<rating>`, `<mbti>`, `signal_<rank_signal>`); an overlay that the manifest does not hold refuses with `asset_missing` and writes no PDF. Tests: test_r2_required_assets_are_the_official_overlays, test_r2_missing_overlay_refuses_with_asset_missing.
- R3: The pages are A4, `595.2756 x 841.8898 pt`, 1 template pixel is 0.24 pt and `y_fitz = 841.8898 - y_hpdf`. Each value is printed in its own box of the table "Fields", strictly inside the page, and no two boxes of a page overlap; a value that does not fit its box is shrunk down to 6 pt and then cut with `...`; the comment is wrapped into its box and its last line cut with `...` when the box is full. The area and unit tables hold exactly one row for each area or unit of the input and no other row; a question count of an area or a unit is the number of questions of the input whose `area` or `unit` is its name (0 when none), and no other count is made. Tests: test_r3_fields_fit_their_boxes_inside_a4_without_overlap, test_r3_long_values_shrink_to_6pt_then_are_cut, test_r3_comment_is_wrapped_and_cut_with_an_ellipsis, test_r3_question_rows_columns_and_marks, test_r3_area_and_unit_counts_come_from_the_questions_only, test_r3_scores_marks_and_bars_follow_the_input.
- R4: The `diag` input is one JSON object with exactly the members of the table "`diag` input" and their limits; an unknown or missing member, a member of another JSON type, a value out of its limit or an enumerated value not listed refuses with `input_invalid`. Tests: test_r4_diag_good_input_is_accepted_and_normalised, test_r4_diag_members_are_strict.
- R5: The `new_diag` input holds the ten members of the `diag` input and also `apply_code` (a string of 1 to 64 characters, validated and not printed) and `mbti` (one of the 16 uppercase four-letter types or `other`), under the same refusals as R4. Tests: test_r5_new_diag_adds_apply_code_and_mbti.
- R6: The `formative` input is one JSON object with exactly the members of the table "`formative` input" and their limits, under the same refusals as R4. Tests: test_r6_formative_members_are_strict.
- R7: The `detail` of a refusal of R4 to R6 names the member path only (`areas[1].signal`, `questions[2].no`), never a value or an unknown key of the input (core.md R7). Tests: test_r7_details_hold_no_input_value.
- R8: The same input with the same assets gives the same PDF bytes: the PDF is saved with no new identifier and cleared metadata, and holds no creation or modification time; its only font is NanumGothic (core.md R8, R12). Tests: test_r8_the_plan_is_deterministic, test_r8_same_input_same_bytes_and_no_time_or_id.
- R9: A line-break token of a string of the input (core.md, "Line-break tokens") breaks the comment's line and becomes one space in a one-line value; it is never printed, so no raw `rn` token is planned; an empty string is no field, so the parity check never searches for ''. Tests: test_r9_line_break_tokens_break_and_no_raw_rn_is_planned, test_r3_fields_fit_their_boxes_inside_a4_without_overlap.
- R10: The parity check (`python3 -m studyq_linux.parity --type diag|new_diag|formative --pdf <pdf> --assets <dir> < input.json`, core.md "The parity check") reports 0 mismatches and 0 raw `rn` tokens on the synthetic fixtures over synthetic templates and over the official templates of `STUDYQ_ASSETS_DIR`, and counts a displaced field and a raw `rn`. Tests: test_r10_parity_reports_zero_mismatches_and_zero_raw_rn, test_r10_parity_counts_a_displaced_field_and_a_raw_rn, test_r10_parity_against_the_official_templates, test_r10_parity_module_takes_the_new_types.

## Interface

### Forms

```
python3 -m studyq_linux render --type diag|new_diag|formative --assets <dir> --out <pdf>   < input.json
python3 -m studyq_linux.parity --type diag|new_diag|formative --pdf <pdf> --assets <dir>   < input.json
```

### Template sequences

| Type | Pages |
| --- | --- |
| `diag` | `image/diag/diag_1.jpg` cover, `diag_2.jpg` certificate, `diag_3.jpg` summary, `diag_4.jpg` difficulty, `diag_5.jpg` thinking-skill graph, `diag_6.jpg` area table, `diag_7.jpg` and `diag_8.jpg` (no field of the input), `diag_9.jpg` questions 1 to 25, then `diag_9.jpg` for each further 25 |
| `new_diag` | `image/new_diag/diag_1.jpg` to `diag_8.jpg` as `diag`, `diag_9.jpg` questions 1 to 25, `diag_10.jpg` learning style, then `diag_10.jpg` for each further 25 questions (the rows of the question table) |
| `formative` | `image/formative/formative_1.jpg` cover, `formative_2.jpg` results, `formative_3.jpg` test information, `formative_4.jpg` achievement, `formative_5.jpg` (no field of the input), `formative_6.jpg` questions 1 to 25 (15 in the left table, 10 in the right), then `formative_6.jpg` for each further 25 |

### Overlays

| Type | Overlay | Where |
| --- | --- | --- |
| `diag`, `new_diag` | `image/diag/2p/lv0<grade>.jpg` | summary page, top left at `(190, 380)` px |
| `diag` | `image/diag/4p/area_graph.jpg`; `new_diag`: `image/diag/4p/03_graph.jpg` | graph page at `(248, 1300)` px; `image/diag/4p/oneself-point.png` on the axis of each of the first four areas (left, top, right, bottom) at `378 px x score / 100` from the centre `(992, 995)` of the graph |
| `diag` | `image/diag/5p/light_<signal>.jpg` | area table, signal column of the area's row |
| `diag`, `new_diag` | `image/diag/6p/<rating>.jpg` | area page, one card for each area, 4 a row |
| `diag` | `image/diag/8p/difficulty_chart.jpg` | difficulty page at `(281, 1775)` px |
| `new_diag` | `image/diag/9p/<mbti>.jpg` | learning-style page, inside `(227, 262, 2258, 718)` px, never enlarged |
| `formative` | `image/formative/2p/score_chart.jpg` | results page at `(212, 1435)` px, with a bar for the score and the average |
| `formative` | `image/formative/4p/03 graph.jpg`, `03 average-point.png`, `03 oneself-point.png` | achievement page at `(236, 520)` px; the two points on the axis of each of the first four units (top, right, bottom, left) at `483 px x value / 100` from the centre `(805, 632)` of the graph |
| `formative` | `image/formative/6p/signal_<rank_signal>.jpg` | first question page, at `(1798, 140)` px |
| all | `image/mark_o.jpg`, `image/mark_x.jpg` | the result cell of each question row, by `correct` |

### Fields

Boxes in template pixels `(x0, y0, x1, y1)`; sizes in pt. `p<n>` is the page number.

| Field | Box | Align, size |
| --- | --- | --- |
| `p1.title` | `diag`, `new_diag`: `(866, 1312, 2345, 1417)`; `formative`: `(779, 1286, 2258, 1400)` | left, 18 |
| `p1.test_date`, `p1.student` | `(1732, 3048, 2345, 3136)`, `(1732, 3141, 2345, 3229)` | left, 14 |
| `p2.student` (`diag`, `new_diag`) | `(630, 1473, 2100, 1554)` | left, 16 |
| `p3.total_score`, `p3.avg_score` (`diag`, `new_diag`) | `(1295, 691, 1636, 796)`, `(1295, 1120, 1636, 1225)`; bars from 311.5 px, 18.4975 px a point | right, 24 |
| `p3.comment.<k>` (`diag`, `new_diag`), `p2.comment.<k>` (`formative`) | lines of 17.6 pt in `(912, 2640, 2210, 3240)`, `(945, 2538, 2240, 3238)` | left, 11 |
| `p4.<level>.questions`, `p4.<level>.score` (`diag`, `new_diag`) | rows from `(1715, 726, 2108, 806)`; scores centred on `(548, 1008, 1467, 1925)` px at 1997 px | right 16, centre 20 |
| `p5.a<i>.questions` (`diag`, `new_diag`) | the question count of area `i` (0 to 3) in its bubble of the graph | centre, 20 |
| `p6.a<i>.name`, `.correct`, `.total`, `.caption` (`diag`, `new_diag`) | the printed rows `(222, 670, 2296, 1057)` are masked white and the areas drawn in that body, one row each; `.caption` under the area's card | left or centre, at most 11 |
| `p2.rank_signal`, `p2.my_score`, `p2.avg_score` (`formative`) | `(376, 525, 709, 753)`, `(1330, 639, 1654, 744)`, `(1330, 1033, 1654, 1138)` | centre 28, right 24 |
| `p3.questions`, `p3.<level>.questions`, `p3.u<i>.questions` (`formative`) | `(1330, 472, 2091, 578)`; level rows from `(1575, 2581, 2156, 2660)`; unit bubbles of the first four units | right 20, right 16, centre 20 |
| `p4.u<i>.name`, `.my_score`, `.average`, `p4.<level>.score` (`formative`) | the unit list in `(1636, 1220, 2236, 1840)`; level bars from `(409.5, 3250)` px, 7.42 px a point | at most 9; centre 14 |
| `p<n>.q<i>.no`, `.area`, `.difficulty`, `.correct_rate` (`diag`, `new_diag`) | rows from 334 px, 98 px a row, columns `no` 247-425, `area` 432-1297, `difficulty` 1304-1517, `correct_rate` 1750-1999 px | centre (area left), 10.5 |
| `p<n>.q<i>.no`, `.difficulty`, `.correct_rate` (`formative`) | rows from 1794 px, 98 px a row, columns 212-394, 399-592, 822-998 px, the right table 1104 px to the right; `unit` has no column and is not printed | centre, 10.5 |

### `diag` input

One JSON object with exactly these members (an example of the shapes is in the tests, with
invented values):

| Member | Value |
| --- | --- |
| `title`, `student`, `test_date` | strings of 1 to 120, 1 to 64 and 1 to 32 characters |
| `grade` | an integer from 1 to 9 |
| `total_score`, `avg_score` | numbers from 0 to 100 |
| `comment` | a string of 0 to 3000 characters with line-break tokens; may be empty |
| `areas` | 1 to 8 objects `{name, score, average, signal, rating}`: `name` 1 to 80 characters; `score`, `average` 0 to 100; `signal` `green`, `yellow` or `red`; `rating` `nice`, `normal`, `base` or `fail` |
| `difficulty` | an object of `top`, `high`, `mid`, `low`, each `{questions, score}`: `questions` an integer from 0 to 500, `score` 0 to 100 |
| `questions` | 1 to 100 objects `{no, area, difficulty, correct_rate, correct}`: `no` 1 to 500, `area` 1 to 80 and `difficulty` 1 to 32 characters, `correct_rate` 0 to 100, `correct` a boolean |

### `new_diag` input

The members of the `diag` input, and `apply_code` (1 to 64 characters) and `mbti` (`ENFJ`, `ENFP`,
`ENTJ`, `ENTP`, `ESFJ`, `ESFP`, `ESTJ`, `ESTP`, `INFJ`, `INFP`, `INTJ`, `INTP`, `ISFJ`, `ISFP`, `ISTJ`,
`ISTP` or `other`).

### `formative` input

| Member | Value |
| --- | --- |
| `title`, `student`, `test_date` | as `diag` |
| `my_score`, `avg_score` | numbers from 0 to 100 |
| `rank_signal` | an integer from 1 to 5 |
| `comment` | as `diag` |
| `units` | 1 to 12 objects `{name, my_score, average}`: `name` 1 to 80 characters, the scores 0 to 100 |
| `difficulty` | as `diag` |
| `questions` | 1 to 100 objects `{no, unit, difficulty, correct_rate, correct}`, as `diag` with `unit` for `area` |

### Tests

`modules/studyq-linux/tests/test_reports_batch1.py` (and `test_r5_type_unknown` of `test_core.py`),
with synthetic fixtures of invented values only. The pure-Python tests run in `./check`
unconditionally; the rendering tests run under core.md R27, and the parity test against the
official templates only with `STUDYQ_ASSETS_DIR`.

## Not included

- The report types `ganga`, `na_score` and `smart_book`; they are a later revision.
- How the TypeScript server reads the input from the LMS; that is
  [direct-key-lookup.md](../academy-mcp/direct-key-lookup.md) R8 and the tool's own contract.
- Areas or units beyond the first four on the graphs: the graphs of the templates have four
  axes; every area and unit is in its table.
- The unit tables of `formative_6.jpg`, the per-session page `formative_5.jpg` and the unit pages
  `diag_7.jpg`, `diag_8.jpg`: the inputs hold no data for them, and nothing is invented.
- Pixel equality with the PDFs of `StudyQReport.exe`: only the field placement of the parity check
  is promised.
