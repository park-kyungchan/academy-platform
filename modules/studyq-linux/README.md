# studyq-linux

The Linux StudyQ PDF executor, `python3 -m studyq_linux`. Its contract is
[`specs/studyq-linux/core.md`](../../specs/studyq-linux/core.md).

```
PYTHONPATH=modules/studyq-linux/src python3 -m studyq_linux render --type course_book --assets <dir> --out <pdf> < input.json
PYTHONPATH=modules/studyq-linux/src python3 -m studyq_linux.parity --pdf <pdf> --assets <dir> < input.json
```

`render` needs PyMuPDF (`import pymupdf`) and an assets directory with a verified `manifest.json`
(`image/...`, `fonts/NanumGothic-Regular.ttf`); it makes no network call. `render --type cism` is built (R18, R19), `render --type diag|new_diag|formative`
([`reports-batch1.md`](../../specs/studyq-linux/reports-batch1.md)) and `render --type ganga|na_score|smart_book`
([`reports-batch2.md`](../../specs/studyq-linux/reports-batch2.md));
`assemble-print --out <pdf>` (R20-R25) reads local `print.zip` archives under the `archive_root` of its input, with the key
from the 0600 file named by `STUDYQ_PRINT_KEY_FILE`, and makes no network call. The parity module takes an
optional `--type` of the eight report types (default `course_book`).

Tests: `python3 -B -m unittest discover -s modules/studyq-linux/tests` (the rendering tests run when
`CHECK_STUDYQ_PYMUPDF=1` is set or `pymupdf` imports; `STUDYQ_ASSETS_DIR=<dir>` adds the parity test
against the official templates). The input is synthetic in every test; no template, archive or
student data is committed here.
