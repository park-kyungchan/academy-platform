# Textbook TOC data

`academy_textbook_toc_get` answers the units, pages and chapter assessments of every catalog sample book from `data/textbook-toc.json` (schema `academy.textbook-toc/1`). It makes no LMS request, no network call, no PDF download and no image read; `bun build` inlines the data into the endpoint bundle. Use it before `academy_textbook_units_get` or `academy_textbook_pdf`.

## A record

| Member | Meaning |
| --- | --- |
| `book` | Canonical name, DailyRecord rule M3: brand, `중` for middle school, `grade-term`, `-volume` when the title states one (`가우스 5-1-2`, `오일러 4-2`, `가우스 중1-1-2`). High school: brand and subject, then `N권` when the subject has several sample files (`가우스 공통수학1 1권`, `시그마 수학(상)`). |
| `aliases` | Other names: the series word (`발전`, `최상위`), the catalog brand (`시그마` for `개념연산`), subject spellings (`미적분Ⅰ`). |
| `pdfTitle`, `schoolLevel`, `filename`, `sourceSha256` | The PDF's first outline item, the catalog level, the catalog file name, the SHA-256 of the PDF read. |
| `toc` | The text of the TOC page or pages, layout kept, trailing spaces and the empty `월 일` plan columns removed; or null. |
| `units` | Where the TOC text supports it: `{chapter, chapterTitle, code, title, pages}`, `pages` as printed (`"158"`, `"14~19"`); a chapter assessment or `실전 Master` has `code: null`. `chapterTitle` is null where the TOC prints only codes. Else null. |
| `outline` | When there is no TOC text: the outline items that name units, markers removed. |
| `noTextToc` | True when `toc` is null: no page among the PDF's pages 1-16 lists units with pages. The tool answers it, with the outline items if any, and the session asks the teacher for pages. |

The input `book` takes a canonical name, an LMS bound-book label (`(22개정) 초5-1 가우스 2권 초5-1 발전 2권(2022개정)`), grade and term words (`가우스 5학년 1학기 2권`, `초5-1 가우스 2권`, `가우스 중1 1학기` or `중학교 1학년 1학기`), a name without volume (every volume, at most 4 records) or a file name; `unit` (`6` or `6-5`) keeps chapter 6 only. `not_found` lists up to 20 canonical names of the same brand or grade. A name with `플러스` is never answered with a 가우스 book.

Private-use glyphs of the PDF text (for example ` cm` for cm²) are kept as extracted.

## Rebuild

1. Extract the text layer of each catalog sample PDF into one directory, one `<filename>.json` each: `schoolLevel`, `filename`, `sourceSha256`, `pdfTitle`, `outline` (`page`, `text`) and `tocPages` (`pdfPage`, `text` of `pdftotext -layout`) with the candidate pages; the script picks the TOC pages among them. Keep the PDF only while it is read.
2. `bun scripts/build-textbook-toc.ts --raw <dir>` writes `data/textbook-toc.json`. The same input gives the same bytes.

A TOC page is a page with `쪽수` and `학습 계획` (the plan table), or a page among the first 12 with at least three `title  page` lines (dotted leaders allowed) and no exercise wording. A plan grid page (`1회차  15`, a count of problems per session, not a page) is never a TOC page. Units are parsed from the plan tables (`N. title  page` in a chapter column) and from coded lists (`1-1 title ---- 10`, one or two columns); any line the parser does not understand leaves `units` null for that book, and its `toc` text still answers.

The data of 2026-11-17 covers the 147 sample PDFs of the elementary, middle and high school catalogs. Its candidate pages: every page from 1 to 16 for the 91 books in which a first pass by TOC words (`쪽수`, `차례`, `목차`, `CONTENTS`, `학습 계획`, dotted leaders) over pages 1 to 24 found no TOC page; for the other 56, the pages that pass found.
