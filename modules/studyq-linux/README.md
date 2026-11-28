# studyq-linux

The Linux StudyQ print executor, `python3 -m studyq_linux`. Its contract is
[`specs/studyq-linux/core.md`](../../specs/studyq-linux/core.md).

```
PYTHONPATH=modules/studyq-linux/src python3 -m studyq_linux assemble-print --out <pdf> < input.json
```

`assemble-print --out <pdf>` (R20-R25) reads local `print.zip` archives under the `archive_root` of its input, with
the key from the 0600 file named by `STUDYQ_PRINT_KEY_FILE`, and makes no network call; it needs PyMuPDF
(`import pymupdf`). The official report PDFs are not made here: `StudyQReport.exe` makes them on the user's PC
([`specs/academy-mcp/pc-executor.md`](../../specs/academy-mcp/pc-executor.md)).

Tests: `python3 -B -m unittest discover -s modules/studyq-linux/tests` (the tests that draw a PDF run when
`CHECK_STUDYQ_PYMUPDF=1` is set or `pymupdf` imports). The input is synthetic in every test; no template, archive or
student data is committed here.
