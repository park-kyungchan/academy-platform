"""Tests of the `print.zip` rules of specs/studyq-linux/core.md (R20-R25). Synthetic fixtures, invented values only.

Python's `zipfile` cannot write encrypted archives and the host and CI have no `zip` or `7z`
binary, so this file holds a small ZipCrypto writer. The archive passphrase of every test is
made at run time and kept in a scratch file of mode 0600: no passphrase is in this repository.
The pure-Python tests (input schema, path templates, key file, ZipSlip, size limits, member
selection, page layout) run unconditionally (R26); the tests that draw a PDF run under the gate
of R27 (PyMuPDF).
"""

import json
import os
import struct
import subprocess
import sys
import unittest
import zipfile
import zlib
from unittest import mock

import test_core as core
from studyq_linux import errors
from studyq_linux import print_zip as pz

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def png(width=40, height=30, shade=200, size=None):
    """A valid solid-colour RGB PNG; `size` pads it with trailing bytes to exactly that length."""

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + bytes([shade]) * (width * 3) for _ in range(height))
    data = PNG_MAGIC + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
    if size is not None:
        assert size >= len(data)
        data += b"\x00" * (size - len(data))
    return data


# --- ZipCrypto writer (PKWARE traditional encryption), enough for the tests -------------------


def _crc_table():
    table = []
    for n in range(256):
        c = n
        for _ in range(8):
            c = (c >> 1) ^ 0xEDB88320 if c & 1 else c >> 1
        table.append(c)
    return table


_TABLE = _crc_table()


def _crc_step(crc, byte):
    return ((crc >> 8) ^ _TABLE[(crc ^ byte) & 0xFF]) & 0xFFFFFFFF


class _Cipher:
    def __init__(self, passphrase):
        self.k = [0x12345678, 0x23456789, 0x34567890]
        for byte in passphrase:
            self._update(byte)

    def _update(self, byte):
        self.k[0] = _crc_step(self.k[0], byte)
        self.k[1] = (self.k[1] + (self.k[0] & 0xFF)) & 0xFFFFFFFF
        self.k[1] = (self.k[1] * 134775813 + 1) & 0xFFFFFFFF
        self.k[2] = _crc_step(self.k[2], self.k[1] >> 24)

    def encrypt(self, data):
        out = bytearray()
        for plain in data:
            t = (self.k[2] | 2) & 0xFFFF
            out.append(plain ^ (((t * (t ^ 1)) >> 8) & 0xFF))
            self._update(plain)
        return bytes(out)


def member(name, data, encrypt=True, method=zipfile.ZIP_STORED, attr=0, usize=None):
    return {"name": name, "data": data, "encrypt": encrypt, "method": method, "attr": attr, "usize": usize}


def zip_bytes(members, passphrase):
    """A ZIP archive; a member with `encrypt` is ZipCrypto-encrypted under `passphrase`."""
    out = bytearray()
    central = bytearray()
    for m in members:
        name = m["name"].encode("utf-8")
        data = m["data"]
        crc = zlib.crc32(data) & 0xFFFFFFFF
        if m["method"] == zipfile.ZIP_DEFLATED:
            comp = zlib.compressobj(9, zlib.DEFLATED, -15)
            body = comp.compress(data) + comp.flush()
        else:
            body = data
        flags = 0
        if m["encrypt"]:
            flags = 1
            header = bytes(range(11)) + bytes([crc >> 24])
            body = _Cipher(passphrase).encrypt(header + body)
        usize = len(data) if m["usize"] is None else m["usize"]
        offset = len(out)
        out += struct.pack("<IHHHHHIIIHH", 0x04034B50, 20, flags, m["method"], 0, 33, crc, len(body), usize, len(name), 0) + name + body
        central += struct.pack("<IHHHHHHIIIHHHHHII", 0x02014B50, (3 << 8) | 20, 20, flags, m["method"], 0, 33, crc, len(body), usize, len(name), 0, 0, 0, 0, m["attr"], offset) + name
    cd_offset = len(out)
    out += central
    out += struct.pack("<IHHHHIIH", 0x06054B50, 0, 0, len(members), len(members), len(central), cd_offset, 0)
    return bytes(out)


def pdf_bytes(pages):
    doc = core.pymupdf.open()
    for _ in range(pages):
        doc.new_page(width=200, height=300)
    try:
        return doc.tobytes()
    finally:
        doc.close()


class PrintCase(core.TempDirCase):
    def setUp(self):
        super().setUp()
        self.root = os.path.join(os.path.realpath(self.tmp), "archives")
        os.makedirs(self.root)
        self.phrase = os.urandom(8).hex().encode("ascii")
        self.key_file = self.path("print.key")
        self.write_key(self.phrase + b"\r\n")
        patcher = mock.patch.dict(os.environ, {pz.KEY_ENV: self.key_file})
        patcher.start()
        self.addCleanup(patcher.stop)

    def write_key(self, data, mode=0o600, path=None):
        path = path or self.key_file
        if os.path.lexists(path):
            os.unlink(path)
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
        with os.fdopen(fd, "wb") as handle:
            handle.write(data)
        os.chmod(path, mode)
        return path

    def put(self, rel, members, phrase=None):
        path = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as handle:
            handle.write(zip_bytes(members, self.phrase if phrase is None else phrase))
        return path

    def put_exam(self, key, members, **kw):
        return self.put(pz.exam_rel_path(key), members, **kw)

    def exam_doc(self, items, kind="exam"):
        return {"archive_root": self.root, "kind": kind, "items": items}

    def run_print(self, doc, out=None):
        out = out or self.path("out.pdf")
        status, line = core.run_cli(["assemble-print", "--out", out], json.dumps(doc).encode())
        return status, json.loads(line), out

    def refusal(self, doc):
        status, answer, out = self.run_print(doc)
        self.assertEqual(status, 1, answer)
        self.assertEqual(answer["status"], "error")
        self.assertIn(answer["code"], errors.CODES)
        self.assertFalse(os.path.exists(out))
        return answer["code"], answer["detail"]

    def parts(self, items, doc_kind="exam"):
        return pz.collect_parts(self.root, [(pz.exam_rel_path(k), sel, a, e) for k, sel, a, e in items], self.phrase)


class Schema(PrintCase):
    def test_r21_path_templates_follow_the_digit_splits(self):
        self.assertEqual(pz.exam_rel_path(0), "data/contents/exam/d6-000/d4-00/d2-00/ek-0/0-print.zip")
        self.assertEqual(pz.exam_rel_path(7), "data/contents/exam/d6-000/d4-00/d2-00/ek-7/7-print.zip")
        self.assertEqual(pz.exam_rel_path(1_234_567), "data/contents/exam/d6-001/d4-23/d2-45/ek-1234567/1234567-print.zip")
        self.assertEqual(pz.exam_rel_path(123_456_789), "data/contents/exam/d6-123/d4-45/d2-67/ek-123456789/123456789-print.zip")
        self.assertEqual(pz.exam_rel_path(999_999_999), "data/contents/exam/d6-999/d4-99/d2-99/ek-999999999/999999999-print.zip")
        self.assertEqual(pz.examtext_rel_path(0), "data/contents/examtext/d4-00/d2-00/etk-0/0-print.zip")
        self.assertEqual(pz.examtext_rel_path(123_456), "data/contents/examtext/d4-12/d2-34/etk-123456/123456-print.zip")
        self.assertEqual(pz.examtext_rel_path(999_999), "data/contents/examtext/d4-99/d2-99/etk-999999/999999-print.zip")
        for bad in (-1, 1_000_000_000, True, 7.0, "7", None):
            with self.assertRaises(StudyqError_, msg=repr(bad)):
                pz.exam_rel_path(bad)
        for bad in (-1, 1_000_000, False, 7.0, "7", None):
            with self.assertRaises(StudyqError_, msg=repr(bad)):
                pz.examtext_rel_path(bad)

    def test_r21_keys_out_of_range_refuse_input_invalid_and_name_the_member_only(self):
        cases = [
            (self.exam_doc([{"lecture_key": -1, "sel_type": "obj"}]), "range:items[0].lecture_key"),
            (self.exam_doc([{"lecture_key": 1_000_000_000, "sel_type": "obj"}]), "range:items[0].lecture_key"),
            (self.exam_doc([{"lecture_key": 5, "sel_type": "obj"}, {"lecture_key": 2**32, "sel_type": "obj"}]), "range:items[1].lecture_key"),
            (self.exam_doc([{"examtext_key": -1, "sel_type": "obj"}], "examtext"), "range:items[0].examtext_key"),
            (self.exam_doc([{"examtext_key": 1_000_000, "sel_type": "des"}], "examtext"), "range:items[0].examtext_key"),
            (self.exam_doc([{"lecture_key": "12", "sel_type": "obj"}]), "type:items[0].lecture_key"),
            (self.exam_doc([{"lecture_key": True, "sel_type": "obj"}]), "type:items[0].lecture_key"),
            (self.exam_doc([{"lecture_key": 1.0, "sel_type": "obj"}]), "type:items[0].lecture_key"),
        ]
        for doc, detail in cases:
            self.assertEqual(self.refusal(doc), ("input_invalid", detail), doc)

    def test_r20_exam_and_examtext_input_schema_is_strict(self):
        good = {"lecture_key": 5, "sel_type": "obj"}
        cases = [
            ({"kind": "exam", "items": [good]}, "missing:archive_root"),
            ({"archive_root": self.root, "items": [good]}, "missing:kind"),
            ({"archive_root": self.root, "kind": "exam"}, "missing:items"),
            ({"archive_root": self.root, "kind": "other", "items": [good]}, "range:kind"),
            ({"archive_root": self.root, "kind": 3, "items": [good]}, "type:kind"),
            ({"archive_root": self.root, "kind": "exam", "items": [good], "extra": 1}, "unknown_member:input"),
            ({"archive_root": self.root, "kind": "exam", "items": {}}, "type:items"),
            ({"archive_root": self.root, "kind": "exam", "items": []}, "range:items"),
            ({"archive_root": self.root, "kind": "exam", "items": [good] * 201}, "range:items"),
            ({"archive_root": self.root, "kind": "exam", "items": [{"lecture_key": 5}]}, "missing:items[0].sel_type"),
            ({"archive_root": self.root, "kind": "exam", "items": [{"sel_type": "obj"}]}, "missing:items[0].lecture_key"),
            ({"archive_root": self.root, "kind": "exam", "items": [{**good, "more": 1}]}, "unknown_member:items[0]"),
            ({"archive_root": self.root, "kind": "exam", "items": [{"examtext_key": 5, "sel_type": "obj"}]}, "missing:items[0].lecture_key"),
            ({"archive_root": self.root, "kind": "examtext", "items": [good]}, "missing:items[0].examtext_key"),
            ({"archive_root": self.root, "kind": "exam", "items": [{**good, "sel_type": "1"}]}, "range:items[0].sel_type"),
            ({"archive_root": self.root, "kind": "exam", "items": [{**good, "sel_type": 1}]}, "type:items[0].sel_type"),
            ({"archive_root": self.root, "kind": "exam", "items": [{**good, "include_answers": "yes"}]}, "type:items[0].include_answers"),
            ({"archive_root": self.root, "kind": "exam", "items": [{**good, "include_explain": 1}]}, "type:items[0].include_explain"),
            ({"archive_root": self.root, "kind": "exam", "items": ["x"]}, "type:items[0]"),
            ({"archive_root": 7, "kind": "exam", "items": [good]}, "type:archive_root"),
            ({"archive_root": "relative/dir", "kind": "exam", "items": [good]}, "type:archive_root"),
            ({"archive_root": self.root + "/missing", "kind": "exam", "items": [good]}, "range:archive_root"),
            ({"archive_root": self.key_file, "kind": "exam", "items": [good]}, "range:archive_root"),
        ]
        link = self.path("link")
        os.symlink(self.root, link)
        cases.append(({"archive_root": link, "kind": "exam", "items": [good]}, "range:archive_root"))
        for doc, detail in cases:
            self.assertEqual(self.refusal(doc), ("input_invalid", detail), (doc, detail))
        # The refusals open no file of the archives or the key.
        with mock.patch.object(pz.zipfile, "ZipFile", side_effect=AssertionError("archive opened")), mock.patch.object(pz, "read_key", side_effect=AssertionError("key read")):
            for doc, _ in cases:
                self.assertEqual(self.refusal(doc)[0], "input_invalid")

    def test_r20_a_good_input_is_normalised_with_defaults(self):
        root, kind, items = pz.validate_print_input(
            self.exam_doc([{"lecture_key": 1_234_567, "sel_type": "sbj"}, {"lecture_key": 5, "sel_type": "des", "include_answers": True, "include_explain": True}])
        )
        self.assertEqual((root, kind), (self.root, "exam"))
        self.assertEqual(items, [(pz.exam_rel_path(1_234_567), "sbj", False, False), (pz.exam_rel_path(5), "des", True, True)])
        _, kind, items = pz.validate_print_input(self.exam_doc([{"examtext_key": 123_456, "sel_type": "obj"}], "examtext"))
        self.assertEqual((kind, items), ("examtext", [(pz.examtext_rel_path(123_456), "obj", False, False)]))

    def clinic(self, **over):
        doc = {
            "archive_root": self.root,
            "kind": "clinic",
            "clinic_list": "1|,|2|,|3",
            "exam_key": "10|,|20|,|123456789",
            "content_list": "Alpha|,|Beta|,|Gamma",
            "clinic_type": "clinic",
            "lecture_value": "10|,|20|,|123456789",
        }
        doc.update(over)
        return {k: v for k, v in doc.items() if v is not None}

    def test_r22_clinic_input_resolves_each_exam_key_through_the_exam_template(self):
        _, kind, items = pz.validate_print_input(self.clinic())
        self.assertEqual(kind, "clinic")
        self.assertEqual(items, [(pz.exam_rel_path(k), "obj", False, False) for k in (10, 20, 123_456_789)])
        _, _, items = pz.validate_print_input(self.clinic(sel_types="sbj|,|des|,|obj", include_answers=True, include_explain=True))
        self.assertEqual([i[1:] for i in items], [("sbj", True, True), ("des", True, True), ("obj", True, True)])
        _, _, items = pz.validate_print_input(self.clinic(sel_type="des"))
        self.assertEqual({i[1] for i in items}, {"des"})
        _, _, items = pz.validate_print_input(self.clinic(clinic_list="1", exam_key="0", content_list="x", lecture_value="0"))
        self.assertEqual(items, [(pz.exam_rel_path(0), "obj", False, False)])

    def test_r22_clinic_input_refuses_a_bad_member_and_names_its_path_only(self):
        marker = "zz-marker"
        cases = [
            (self.clinic(clinic_list=None), "missing:clinic_list"),
            (self.clinic(exam_key=None), "missing:exam_key"),
            (self.clinic(content_list=None), "missing:content_list"),
            (self.clinic(clinic_type=None), "missing:clinic_type"),
            (self.clinic(lecture_value=None), "missing:lecture_value"),
            (self.clinic(extra=marker), "unknown_member:input"),
            (self.clinic(items=[]), "unknown_member:input"),
            (self.clinic(clinic_list=5), "type:clinic_list"),
            (self.clinic(clinic_type=5), "type:clinic_type"),
            (self.clinic(clinic_type=""), "range:clinic_type"),
            (self.clinic(clinic_list=""), "range:clinic_list"),
            (self.clinic(clinic_list="1|,|2"), "count_mismatch:content_list"),
            (self.clinic(content_list="a|,|b"), "count_mismatch:content_list"),
            (self.clinic(lecture_value="1|,|2|,|3|,|4"), "count_mismatch:lecture_value"),
            (self.clinic(exam_key="1|,|2"), "count_mismatch:exam_key"),
            (self.clinic(clinic_list="1|,||,|3"), "empty_element:clinic_list"),
            (self.clinic(content_list="|,|b|,|c"), "empty_element:content_list"),
            (self.clinic(lecture_value="1|,|2|,|"), "empty_element:lecture_value"),
            (self.clinic(exam_key="1|,|2|,|"), "empty_element:exam_key"),
            (self.clinic(exam_key="1|,|2|,|3a"), "range:exam_key"),
            (self.clinic(exam_key="1|,|2|,|-3"), "range:exam_key"),
            (self.clinic(exam_key="1|,|2|,| 3"), "range:exam_key"),
            (self.clinic(exam_key="1|,|2|,|1000000000"), "range:exam_key"),
            (self.clinic(exam_key="1|,|2|,|007"), "range:exam_key"),
            (self.clinic(sel_types="obj|,|sbj|,|des", sel_type="obj"), "conflict:sel_type"),
            (self.clinic(sel_types="obj|,|sbj"), "count_mismatch:sel_types"),
            (self.clinic(sel_types="obj|,|sbj|,|1"), "range:sel_types"),
            (self.clinic(sel_types=3), "type:sel_types"),
            (self.clinic(sel_type="1"), "range:sel_type"),
            (self.clinic(sel_type=None, include_answers="true"), "type:include_answers"),
            (self.clinic(include_explain=0), "type:include_explain"),
            (self.clinic(clinic_list="|,|".join(str(i) for i in range(201)), exam_key="|,|".join("1" for _ in range(201)),
                         content_list="|,|".join("c" for _ in range(201)), lecture_value="|,|".join("1" for _ in range(201))), "range:clinic_list"),
        ]
        for doc, detail in cases:
            self.assertEqual(self.refusal(doc), ("input_invalid", detail), (doc, detail))
        code, detail = self.refusal(self.clinic(extra=marker, exam_key="zz-marker"))
        self.assertNotIn(marker, detail)
        code, detail = self.refusal(self.clinic(exam_key="zz-marker|,|2|,|3"))
        self.assertNotIn(marker, detail)

    def test_r22_two_hundred_clinic_items_are_accepted(self):
        join = "|,|".join
        n = 200
        doc = self.clinic(clinic_list=join(str(i) for i in range(n)), exam_key=join(str(i) for i in range(n)), content_list=join("c" for _ in range(n)),
                          lecture_value=join(str(i) for i in range(n)))
        self.assertEqual(len(pz.validate_print_input(doc)[2]), n)

    def test_not_implemented_stays_reserved_in_the_error_codes(self):
        self.assertIn("not_implemented", errors.CODES)
        status, line = core.run_cli(["assemble-print", "--out", self.path("p.pdf")], b"{}")
        self.assertEqual(json.loads(line)["code"], "input_invalid")
        self.assertEqual(status, 1)

    def test_r20_an_existing_output_is_refused_before_anything_is_read(self):
        class Poison:
            def read(self, _n=-1):
                raise AssertionError("stdin read")

        out = self.path("taken.pdf")
        with open(out, "wb") as handle:
            handle.write(b"keep")
        from studyq_linux import cli

        status, line = cli.run(["assemble-print", "--out", out], Poison())
        self.assertEqual((status, json.loads(line)["code"]), (1, "output_exists"))
        with open(out, "rb") as handle:
            self.assertEqual(handle.read(), b"keep")


class KeyFile(PrintCase):
    def key_refusal(self):
        self.put_exam(5, [member("print-obj-exam.png", png())])
        with mock.patch.object(pz.zipfile, "ZipFile", side_effect=AssertionError("archive opened before the key was qualified")):
            code, detail = self.refusal(self.exam_doc([{"lecture_key": 5, "sel_type": "obj"}]))
        self.assertEqual(code, "archive_key_missing", detail)
        self.assertNotIn(self.phrase.decode(), detail)
        self.assertLessEqual(len(detail.encode()), 256)

    def test_r23_a_missing_variable_or_file_refuses_before_any_archive_is_opened(self):
        with mock.patch.dict(os.environ):
            del os.environ[pz.KEY_ENV]
            self.key_refusal()
        with mock.patch.dict(os.environ, {pz.KEY_ENV: ""}):
            self.key_refusal()
        with mock.patch.dict(os.environ, {pz.KEY_ENV: self.path("absent.key")}):
            self.key_refusal()
        with mock.patch.dict(os.environ, {pz.KEY_ENV: self.tmp}):
            self.key_refusal()

    def test_r23_a_file_of_another_mode_owner_or_size_refuses(self):
        for mode in (0o644, 0o640, 0o604, 0o660, 0o602, 0o666):
            self.write_key(self.phrase, mode)
            self.key_refusal()
        for data in (b"", b"\r\n", b"\n", b"x" * 257):
            self.write_key(data)
            self.key_refusal()
        self.write_key(self.phrase)
        with mock.patch.object(pz.os, "geteuid", return_value=os.geteuid() + 1):
            self.key_refusal()
        # A symbolic link to a good file is not a regular file of the caller.
        target = self.write_key(self.phrase, path=self.path("target.key"))
        os.unlink(self.key_file)
        os.symlink(target, self.key_file)
        self.key_refusal()

    def test_r23_a_qualified_file_gives_the_key_without_its_trailing_line_break_only(self):
        for data, expected in ((self.phrase, self.phrase), (self.phrase + b"\n", self.phrase), (self.phrase + b"\r\n", self.phrase),
                               (b" " + self.phrase + b" ", b" " + self.phrase + b" "), (b"x" * 256, b"x" * 256)):
            self.write_key(data, 0o600)
            self.assertEqual(pz.read_key({pz.KEY_ENV: self.key_file}), expected)
        self.write_key(self.phrase, 0o400)
        self.assertEqual(pz.read_key({pz.KEY_ENV: self.key_file}), self.phrase)

    def test_r23_a_wrong_key_refuses_and_no_key_is_printed(self):
        other = os.urandom(8).hex().encode("ascii")
        self.put_exam(5, [member("print-obj-exam.png", png())], phrase=other)
        doc = {"archive_root": self.root, "kind": "exam", "items": [{"lecture_key": 5, "sel_type": "obj"}]}
        out = self.path("o.pdf")
        env = dict(os.environ, PYTHONPATH=os.path.join(core.MODULE, "src"))
        proc = subprocess.run([sys.executable, "-B", "-m", "studyq_linux", "assemble-print", "--out", out], input=json.dumps(doc).encode(), env=env, capture_output=True)
        self.assertEqual(proc.returncode, 1)
        lines = proc.stdout.decode().splitlines()
        self.assertEqual(len(lines), 1)
        answer = json.loads(lines[0])
        self.assertEqual((answer["status"], answer["code"]), ("error", "archive_refused"))
        for secret in (self.phrase, other):
            self.assertNotIn(secret, proc.stdout)
            self.assertNotIn(secret, proc.stderr)
        self.assertFalse(os.path.exists(out))


class Archives(PrintCase):
    def test_r20_members_are_chosen_by_the_selection_and_kept_in_member_order(self):
        names = ["print-obj-explain.png", "print-obj-exam.png", "notes.txt", "print-obj-answer.png", "print-obj-objective.png", "print-sbj-exam.png", "print-des-exam.png"]
        data = {name: png(shade=40 + 20 * i) for i, name in enumerate(names)}
        self.put_exam(11, [member(n, data[n]) for n in names])
        got = lambda sel, a, e: [d for _, d in self.parts([(11, sel, a, e)])]
        self.assertEqual(got("obj", False, False), [data["print-obj-exam.png"], data["print-obj-objective.png"]])
        self.assertEqual(got("obj", True, False), [data["print-obj-exam.png"], data["print-obj-answer.png"], data["print-obj-objective.png"]])
        self.assertEqual(got("obj", True, True), [data[n] for n in ("print-obj-explain.png", "print-obj-exam.png", "print-obj-answer.png", "print-obj-objective.png")])
        self.assertEqual(got("obj", False, True), [data["print-obj-explain.png"], data["print-obj-exam.png"], data["print-obj-objective.png"]])
        # `objective` belongs to `obj` only.
        self.assertEqual(got("sbj", True, True), [data["print-sbj-exam.png"]])
        self.assertEqual(got("des", False, False), [data["print-des-exam.png"]])
        self.assertEqual({k for k, _ in self.parts([(11, "obj", True, True)])}, {"png"})

    def test_r20_archives_are_read_in_the_order_of_the_input_list(self):
        self.put_exam(1, [member("print-obj-exam.png", png(shade=10))])
        self.put_exam(2, [member("print-obj-exam.png", png(shade=20))])
        self.put_exam(3, [member("print-sbj-exam.png", png(shade=30))])
        got = [d for _, d in self.parts([(3, "sbj", False, False), (1, "obj", False, False), (2, "obj", False, False), (1, "obj", False, False)])]
        self.assertEqual(got, [png(shade=30), png(shade=10), png(shade=20), png(shade=10)])

    def test_r20_pdf_members_are_taken_whole_and_never_mixed_with_png_members(self):
        fake = lambda tag: b"%PDF-1.4 " + tag
        self.put_exam(1, [member("b.pdf", fake(b"b")), member("readme.txt", b"x"), member("a.PDF", fake(b"a"))])
        self.assertEqual(self.parts([(1, "des", True, True)]), [("pdf", fake(b"b")), ("pdf", fake(b"a"))])
        self.put_exam(2, [member("print-obj-exam.png", png()), member("x.pdf", fake(b"x"))])
        self.put_exam(3, [member("notes.txt", b"x"), member("print-sbj-exam.png", png())])
        self.put_exam(4, [member("print-obj-exam.png", b"\x89PNX" + b"0" * 40)])
        self.put_exam(5, [member("x.pdf", b"%PDX-1.4")])
        for key, sel in ((2, "obj"), (3, "obj"), (4, "obj"), (5, "obj")):
            with self.assertRaises(StudyqError_) as caught:
                self.parts([(key, sel, False, False)])
            self.assertEqual(caught.exception.code, "archive_refused", key)

    def test_r20_a_missing_archive_refuses_before_any_archive_is_read(self):
        self.put_exam(1, [member("print-obj-exam.png", png())])
        doc = self.exam_doc([{"lecture_key": 1, "sel_type": "obj"}, {"lecture_key": 2, "sel_type": "obj"}])
        with mock.patch.object(pz.zipfile.ZipFile, "open", side_effect=AssertionError("member read")):
            self.assertEqual(self.refusal(doc)[0], "archive_missing")
        doc = self.exam_doc([{"examtext_key": 9, "sel_type": "obj"}], "examtext")
        self.assertEqual(self.refusal(doc), ("archive_missing", "archive not found"))

    def test_r20_an_archive_that_is_a_link_or_corrupt_is_refused(self):
        path = self.put_exam(1, [member("print-obj-exam.png", png())])
        self.put_exam(2, [member("print-obj-exam.png", png())])
        os.unlink(pz_path(self.root, 2))
        os.symlink(path, pz_path(self.root, 2))
        os.makedirs(os.path.dirname(pz_path(self.root, 3)), exist_ok=True)
        with open(pz_path(self.root, 3), "wb") as handle:
            handle.write(b"not a zip archive at all")
        for key in (2, 3):
            self.assertEqual(self.refusal(self.exam_doc([{"lecture_key": key, "sel_type": "obj"}]))[0], "archive_refused", key)

    def test_r24_unsafe_member_names_refuse_the_whole_run_before_any_member_is_read(self):
        bad = ["/abs.png", "\\back.png", "../x.png", "a/../../x.png", "a/b/../../../x.png", "C:evil.png", "c:\\x.png", "..", "a\\..\\..\\x.png"]
        for name in bad:
            self.put_exam(1, [member("print-obj-exam.png", png()), member(name, b"data")])
            with mock.patch.object(pz.zipfile.ZipFile, "open", side_effect=AssertionError("member read")):
                code, _ = self.refusal(self.exam_doc([{"lecture_key": 1, "sel_type": "obj"}]))
            self.assertEqual(code, "archive_refused", name)
            os.unlink(pz_path(self.root, 1))
        # A symbolic-link member, whatever its name.
        self.put_exam(1, [member("print-obj-exam.png", png()), member("link.txt", b"../../x", attr=0o120777 << 16)])
        with mock.patch.object(pz.zipfile.ZipFile, "open", side_effect=AssertionError("member read")):
            self.assertEqual(self.refusal(self.exam_doc([{"lecture_key": 1, "sel_type": "obj"}]))[0], "archive_refused")
        # A harmless nested name and a regular file mode are fine.
        self.put_exam(1, [member("sub/dir/print-obj-exam.png", png(), attr=0o100644 << 16), member("odd..name.txt", b"x")])
        self.assertEqual(len(self.parts([(1, "obj", False, False)])), 1)

    def test_r25_more_than_4096_members_refuse_before_any_member_is_read(self):
        many = [member("f%d" % i, b"") for i in range(4096)]
        self.put_exam(1, many[:-1] + [member("print-obj-exam.png", png())])
        self.assertEqual(len(self.parts([(1, "obj", False, False)])), 1)
        self.put_exam(1, many + [member("print-obj-exam.png", png())])
        with mock.patch.object(pz.zipfile.ZipFile, "open", side_effect=AssertionError("member read")):
            self.assertEqual(self.refusal(self.exam_doc([{"lecture_key": 1, "sel_type": "obj"}])), ("archive_refused", "too many members"))

    def test_r25_a_member_over_the_limit_refuses_whatever_the_archive_declares(self):
        with mock.patch.object(pz, "MAX_MEMBER_BYTES", 1000), mock.patch.object(pz, "MAX_ARCHIVE_BYTES", 10**9):
            self.put_exam(1, [member("print-obj-exam.png", png(size=1000))])
            self.assertEqual(len(self.parts([(1, "obj", False, False)])), 1)
            self.put_exam(1, [member("print-obj-exam.png", png(size=1001))])
            with self.assertRaises(StudyqError_) as caught:
                self.parts([(1, "obj", False, False)])
            self.assertEqual((caught.exception.code, caught.exception.detail), ("archive_refused", "size limit"))
            # A deflate bomb is cut at the first chunk over the limit: about the limit is decompressed, not the whole.
            bomb = png(size=1000) + b"\x00" * (40 * 1024 * 1024)
            self.put_exam(1, [member("print-obj-exam.png", bomb, encrypt=False, method=zipfile.ZIP_DEFLATED)])
            produced = [0]
            real_read = zipfile.ZipExtFile.read

            def counting(handle, n=-1):
                chunk = real_read(handle, n)
                produced[0] += len(chunk)
                return chunk

            with mock.patch.object(zipfile.ZipExtFile, "read", counting):
                with self.assertRaises(StudyqError_) as caught:
                    self.parts([(1, "obj", False, False)])
            self.assertEqual(caught.exception.code, "archive_refused")
            self.assertLessEqual(produced[0], 1000 + pz.CHUNK)
            # A lying declared size is no way round: the actual bytes count.
            self.put_exam(1, [member("print-obj-exam.png", png(size=5000), encrypt=False, method=zipfile.ZIP_DEFLATED, usize=10)])
            with self.assertRaises(StudyqError_) as caught:
                self.parts([(1, "obj", False, False)])
            self.assertEqual(caught.exception.code, "archive_refused")

    def test_r25_members_over_the_archive_limit_together_refuse(self):
        with mock.patch.object(pz, "MAX_MEMBER_BYTES", 1000), mock.patch.object(pz, "MAX_ARCHIVE_BYTES", 2500):
            members = [member("print-obj-exam.png", png(size=900)), member("a/print-obj-objective.png", png(size=900)), member("b/print-obj-exam.png", png(size=700))]
            self.put_exam(1, members)
            self.assertEqual(len(self.parts([(1, "obj", False, False)])), 3)
            self.put_exam(1, members + [member("c/print-obj-exam.png", png(size=701))])
            with self.assertRaises(StudyqError_) as caught:
                self.parts([(1, "obj", False, False)])
            self.assertEqual((caught.exception.code, caught.exception.detail), ("archive_refused", "size limit"))
            # The budget is per archive: two archives of exactly 2500 bytes each are fine.
            self.put_exam(1, members)
            self.put_exam(2, members)
            self.assertEqual(len(self.parts([(1, "obj", False, False), (2, "obj", False, False)])), 6)

    def test_r25_the_documented_limits_are_the_constants(self):
        self.assertEqual((pz.MAX_MEMBERS, pz.MAX_MEMBER_BYTES, pz.MAX_ARCHIVE_BYTES, pz.CHUNK), (4096, 64 * 1024 * 1024, 256 * 1024 * 1024, 65536))

    def test_r25_a_member_of_64_mib_plus_one_byte_refuses(self):
        bomb = b"\x00" * (64 * 1024 * 1024 + 1)
        self.put_exam(1, [member("print-obj-exam.png", bomb, encrypt=False, method=zipfile.ZIP_DEFLATED)])
        self.assertEqual(self.refusal(self.exam_doc([{"lecture_key": 1, "sel_type": "obj"}])), ("archive_refused", "size limit"))

    def test_r7_refusal_details_hold_no_input_value_and_no_path(self):
        self.put_exam(1, [member("../../" + "marker-name.png", png())])
        for doc in (self.exam_doc([{"lecture_key": 1, "sel_type": "obj"}]), self.exam_doc([{"lecture_key": 2, "sel_type": "obj"}])):
            code, detail = self.refusal(doc)
            self.assertNotIn("marker-name", detail)
            self.assertNotIn(self.tmp, detail)


class Layout(unittest.TestCase):
    def test_r8_images_stack_on_a4_pages_at_the_fitting_width(self):
        h = pz.FIT_WIDTH * 10 / 100
        placed = pz.layout_images([(100, 10)] * 15)
        self.assertEqual([p[0] for p in placed], [0] * 14 + [1])
        self.assertAlmostEqual(placed[0][1], 36.0)
        self.assertAlmostEqual(placed[0][3], 36.0 + 523.2756)
        self.assertAlmostEqual(placed[0][2], 36.0)
        self.assertAlmostEqual(placed[1][2], 36.0 + h)
        self.assertAlmostEqual(placed[14][2], 36.0)
        self.assertLessEqual(placed[13][4], 805.8898)

    def test_r8_a_taller_image_is_scaled_to_the_page_and_fills_it(self):
        (a, b) = pz.layout_images([(1000, 1500), (100, 10)])
        self.assertAlmostEqual(a[4] - a[2], 769.89)
        self.assertAlmostEqual(a[3] - a[1], 769.89 * 1000 / 1500)
        self.assertEqual((a[0], b[0]), (0, 1))
        # An image of exactly the full height leaves no blank page before it.
        (c,) = pz.layout_images([(769890, 523276)][:0] + [(523276, 769890)])
        self.assertEqual(c[0], 0)
        self.assertAlmostEqual(c[2], 36.0)

    def test_r8_a_wide_image_keeps_its_aspect_ratio(self):
        ((page, x0, y0, x1, y1),) = pz.layout_images([(200, 100)])
        self.assertEqual(page, 0)
        self.assertAlmostEqual((y1 - y0) / (x1 - x0), 0.5)


@core.needs_pymupdf
class Assembly(PrintCase):
    def pages_of(self, path):
        doc = core.pymupdf.open(path)
        try:
            return doc.page_count
        finally:
            doc.close()

    def test_r20_exam_archives_become_one_pdf_in_input_order(self):
        self.put_exam(1, [member("print-obj-exam.png", png(100, 10)), member("print-obj-objective.png", png(100, 10)), member("print-obj-answer.png", png(100, 10))])
        self.put_exam(2, [member("print-sbj-exam.png", png(400, 500))])
        doc = self.exam_doc([{"lecture_key": 1, "sel_type": "obj", "include_answers": True}, {"lecture_key": 2, "sel_type": "sbj"}])
        status, answer, out = self.run_print(doc)
        self.assertEqual(status, 0, answer)
        self.assertEqual(list(answer), ["status", "reportType", "pages", "sha256", "outputPath"])
        self.assertEqual((answer["status"], answer["reportType"], answer["pages"], answer["outputPath"]), ("ok", "print_exam", 2, out))
        with open(out, "rb") as handle:
            raw = handle.read()
        import hashlib

        self.assertEqual(hashlib.sha256(raw).hexdigest(), answer["sha256"])
        self.assertEqual(self.pages_of(out), 2)
        # Page 1 holds the three small images, page 2 the scaled-to-width one: sizes follow the layout.
        pdf = core.pymupdf.open(out)
        try:
            first = sorted(info["bbox"][1] for info in pdf[0].get_image_info())
            self.assertEqual(len(first), 3)
            self.assertAlmostEqual(first[0], 36.0, places=1)
            second = pdf[1].get_image_info()
            self.assertEqual(len(second), 1)
            self.assertAlmostEqual(second[0]["bbox"][0], 36.0, places=1)
            self.assertAlmostEqual(second[0]["bbox"][2] - second[0]["bbox"][0], 523.2756, places=1)
        finally:
            pdf.close()

    def test_r20_a_page_overflow_starts_a_new_page(self):
        self.put_exam(1, [member("print-obj-exam.png", png(100, 10)) for _ in range(15)])
        status, answer, out = self.run_print(self.exam_doc([{"lecture_key": 1, "sel_type": "obj"}]))
        self.assertEqual((status, answer["pages"]), (0, 2), answer)

    def test_r8_the_same_input_gives_the_same_bytes_with_no_time_or_id(self):
        self.put_exam(1, [member("print-obj-exam.png", png(60, 40)), member("print-obj-objective.png", png(60, 80, shade=90))])
        doc = self.exam_doc([{"lecture_key": 1, "sel_type": "obj"}])
        _, first, out1 = self.run_print(doc, self.path("a.pdf"))
        _, second, out2 = self.run_print(doc, self.path("b.pdf"))
        self.assertEqual(first["sha256"], second["sha256"])
        with open(out1, "rb") as handle:
            raw = handle.read()
        self.assertNotIn(b"CreationDate", raw)
        self.assertNotIn(b"/ID", raw)

    def test_r20_examtext_and_clinic_kinds_answer_their_report_type(self):
        self.put(pz.examtext_rel_path(123_456), [member("print-des-exam.png", png())])
        status, answer, _ = self.run_print(self.exam_doc([{"examtext_key": 123_456, "sel_type": "des"}], "examtext"))
        self.assertEqual((status, answer["reportType"], answer["pages"]), (0, "print_examtext", 1), answer)
        self.put_exam(10, [member("print-obj-exam.png", png(100, 10))])
        self.put_exam(20, [member("print-sbj-exam.png", png(100, 10)), member("print-sbj-answer.png", png(100, 10))])
        doc = {"archive_root": self.root, "kind": "clinic", "clinic_list": "1|,|2", "exam_key": "10|,|20", "content_list": "A|,|B", "clinic_type": "clinic",
               "lecture_value": "10|,|20", "sel_types": "obj|,|sbj", "include_answers": True}
        status, answer, out = self.run_print(doc, self.path("clinic.pdf"))
        self.assertEqual((status, answer["reportType"], answer["pages"]), (0, "print_clinic", 1), answer)

    def test_r20_pdf_members_are_appended_page_by_page(self):
        self.put_exam(1, [member("a.pdf", pdf_bytes(2)), member("b.pdf", pdf_bytes(1))])
        self.put_exam(2, [member("print-obj-exam.png", png())])
        self.put_exam(3, [member("c.pdf", pdf_bytes(3))])
        doc = self.exam_doc([{"lecture_key": 1, "sel_type": "obj"}, {"lecture_key": 2, "sel_type": "obj"}, {"lecture_key": 3, "sel_type": "obj"}])
        status, answer, out = self.run_print(doc)
        self.assertEqual((status, answer["pages"]), (0, 3 + 1 + 3), answer)
        self.assertEqual(self.pages_of(out), 7)

    def test_r2_a_refused_run_leaves_no_file_and_an_existing_output_is_kept(self):
        doc = self.exam_doc([{"lecture_key": 404, "sel_type": "obj"}])
        status, answer, out = self.run_print(doc)
        self.assertEqual((status, answer["code"], os.path.exists(out)), (1, "archive_missing", False))
        self.put_exam(404, [member("print-obj-exam.png", png())])
        with open(out, "wb") as handle:
            handle.write(b"keep")
        status, answer, _ = self.run_print(doc, out)
        self.assertEqual((status, answer["code"]), (1, "output_exists"))
        with open(out, "rb") as handle:
            self.assertEqual(handle.read(), b"keep")
        self.assertEqual(sorted(os.listdir(self.tmp)), sorted(["archives", "out.pdf", "print.key"]))

    def test_r20_a_member_that_pymupdf_cannot_draw_refuses(self):
        self.put_exam(1, [member("print-obj-exam.png", png()[:40] + b"\x00" * 40)])
        self.put_exam(2, [member("x.pdf", b"%PDF-1.4 truncated")])
        for key in (1, 2):
            status, answer, out = self.run_print(self.exam_doc([{"lecture_key": key, "sel_type": "obj"}]), self.path("bad%d.pdf" % key))
            self.assertEqual((status, answer["code"]), (1, "archive_refused"), answer)
            self.assertFalse(os.path.exists(out))


def pz_path(root, key):
    return os.path.join(root, pz.exam_rel_path(key))


StudyqError_ = errors.StudyqError

if __name__ == "__main__":
    unittest.main()
