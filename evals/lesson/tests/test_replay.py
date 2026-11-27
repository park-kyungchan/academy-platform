import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
REPLAY = os.path.join(os.path.dirname(HERE), "replay")

NAME = "Zorblax Quentin"
NAME2 = "Ulla Brenwick"
SECRET_TEXT = "secret lesson text about fractions"
OTHER_TEXT = "different private words"

HEAD = (
    "| Student id | Student | Course | Course id | Membership | Record key | Attendance value | Attendance "
    "| Progress | Homework | Lesson memo | Daily test | Student memo | Previous record key | Previous date "
    "| Previous progress | Previous homework | Previous lesson memo | Previous homework rate | Cut fields "
    "| Observed at |"
)
SEP = "| " + " | ".join(["---"] * 21) + " |"

FAKE_EDU = """#!{python}
import json, os, sys
here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "calls.log"), "a") as log:
    log.write(" ".join(sys.argv[1:]) + "\\n")
with open(os.path.join(here, "scenarios.json")) as f:
    scenarios = json.load(f)
entry = scenarios.get(sys.argv[4] + "|" + sys.argv[3])
sys.stderr.write("private noise\\n")
if entry is None:
    entry = {{"exit": 2, "stdout": "{{\\"status\\":\\"lesson_student_not_found\\"}}\\n"}}
sys.stdout.write(entry["stdout"])
sys.exit(entry["exit"])
"""


def row(student_id, name, course_id, membership, record, attendance, progress, homework, memo, test, rate):
    cells = [
        student_id, name, "Course", course_id, membership, record, attendance, "Present", progress,
        homework, memo, test, "", "", "", "", "", "", rate, "0", "1970-01-01T00:16:40Z",
    ]
    return "| " + " | ".join(cells) + " |"


def edu_ok(student_id, rows):
    first = json.dumps({"status": "ok", "student_id": student_id, "date": "2031-03-03"})
    table = "<!-- ont:render daily_record -->\n" + HEAD + "\n" + SEP + "\n" + "\n".join(rows) + "\n<!-- /ont:render -->\n"
    return {"exit": 0, "stdout": first + "\n" + table}


def ts_fields(values, course="3", membership="50001", record="41"):
    names = ["progress", "homework", "lessonMemo", "attendance", "dailyTestAssessment", "previousHomeworkAssessment"]
    out = []
    for name in names:
        value = values.get(name)
        target = {"courseId": course, "membershipId": membership, "recordId": record}
        if value is None:
            out.append({"field": name, "target": target,
                        "before": {"state": "unknown", "reasonCode": "unreadable"}})
        else:
            out.append({"field": name, "target": target,
                        "before": {"state": "known", "value": value, "evidenceRefs": ["x"]}})
    return out


def ts_answer(student_id, fields, status="observed"):
    answer = {"status": status, "scope": {"studentId": student_id}, "fields": fields}
    return json.dumps(answer)


def call_lines(n, name, date, answer_text, as_blocks=True, is_error=False):
    use = {"type": "assistant", "message": {"content": [{
        "type": "tool_use", "id": "t%d" % n, "name": "mcp__academy_operations__academy_dailyrecord_get",
        "input": {"displayName": name, "lessonDate": date}}]}}
    content = [{"type": "text", "text": answer_text}] if as_blocks else answer_text
    res = {"type": "user", "message": {"content": [{
        "type": "tool_result", "tool_use_id": "t%d" % n, "content": content, "is_error": is_error}]}}
    return [json.dumps(use), json.dumps(res)]


class ReplayTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.edu = os.path.join(self.dir.name, "edu")
        with open(self.edu, "w") as f:
            f.write(FAKE_EDU.format(python=sys.executable))
        os.chmod(self.edu, 0o755)
        self.scenarios = {}

    def write_scenarios(self):
        with open(os.path.join(self.dir.name, "scenarios.json"), "w") as f:
            json.dump(self.scenarios, f)

    def record(self, lines, name="session.jsonl"):
        path = os.path.join(self.dir.name, name)
        with open(path, "w") as f:
            f.write("\n".join(lines) + "\n")
        return path

    def run_replay(self, *args):
        self.write_scenarios()
        return subprocess.run([sys.executable, "-B", REPLAY, *args], capture_output=True, text=True)

    def edu_calls(self):
        path = os.path.join(self.dir.name, "calls.log")
        if not os.path.exists(path):
            return []
        with open(path) as f:
            return f.read().splitlines()

    def build_session(self):
        values = {"progress": SECRET_TEXT, "homework": "p.41-44", "lessonMemo": "line one line two | pipe",
                  "attendance": "Y", "dailyTestAssessment": "2", "previousHomeworkAssessment": "3"}
        lines = []
        # 1: equal (memo with a line break and a pipe in the tool's answer)
        eq_values = dict(values, lessonMemo="line one\nline two | pipe")
        self.scenarios[NAME + "|2031-03-03"] = edu_ok("77001", [
            row("77001", NAME, "3", "50001", "41", "Y", SECRET_TEXT, "p.41-44", "line one line two \\| pipe", "2", "3")])
        lines += call_lines(1, NAME, "2031-03-03", ts_answer("77001", ts_fields(eq_values)))
        # 2: same pair again: equal, edu is not run a second time
        lines += call_lines(2, NAME, "2031-03-03", ts_answer("77001", ts_fields(eq_values)), as_blocks=False)
        # 3: differs in homework and daily test
        self.scenarios[NAME2 + "|2031-03-03"] = edu_ok("77002", [
            row("77002", NAME2, "3", "50002", "(missing)", "Y", "u3", OTHER_TEXT, "", "9", "3")])
        d_values = dict(values, progress="u3", homework="p.1", lessonMemo="", dailyTestAssessment="2")
        lines += call_lines(3, NAME2, "2031-03-03", ts_answer("77002", ts_fields(d_values, membership="50002", record="42")))
        # 4: not covered: the student has no row (an id is used as it is) -> zero rows
        self.scenarios["77003|2031-03-04"] = edu_ok("77003", [])
        lines += call_lines(4, "77003", "2031-03-04", ts_answer("77003", ts_fields(values)))
        # 5: refused by edu
        lines += call_lines(5, "Nobody Here", "2031-03-05", ts_answer("77009", ts_fields(values)))
        # 6, 7, 8: tool answers that are not observed, an error, no result
        lines += call_lines(6, NAME, "2031-03-06", ts_answer("77001", [], status="unavailable"))
        lines += call_lines(7, NAME, "2031-03-07", "boom", is_error=True)
        lines.append(json.dumps({"type": "assistant", "message": {"content": [{
            "type": "tool_use", "id": "t8", "name": "academy_dailyrecord_get",
            "input": {"displayName": NAME, "lessonDate": "2031-03-08"}}]}}))
        # a call of another tool is not a call
        lines.append(json.dumps({"type": "assistant", "message": {"content": [{
            "type": "tool_use", "id": "t9", "name": "mcp__x__academy_textbook_get", "input": {}}]}}))
        return lines

    def test_every_outcome_and_one_edu_run_per_pair(self):
        path = self.record(self.build_session() + ["{not json", "[1,2"])
        done = self.run_replay("--edu", self.edu, path)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stderr, "")
        self.assertEqual(done.stdout.splitlines(), [
            "call 1 equal",
            "call 2 equal",
            "call 3 differs homework,daily_test_assessment",
            "call 4 not_covered",
            "call 5 refused lesson_student_not_found",
            "call 6 ts_unavailable",
            "call 7 ts_unavailable",
            "call 8 ts_unavailable",
            "unparsed lines 2",
            "total calls 8 equal 2 differs 1 not_covered 1 refused 1 ts_unavailable 3",
        ])
        calls = self.edu_calls()
        self.assertEqual(len(calls), 4, calls)
        self.assertEqual(calls.count("lesson record 2031-03-03 " + NAME), 1)

    def test_output_holds_no_name_text_or_value(self):
        path = self.record(self.build_session())
        for extra in ([], ["--json"]):
            done = self.run_replay("--edu", self.edu, *extra, path)
            self.assertEqual(done.returncode, 0)
            seen = done.stdout + done.stderr
            for word in (NAME, NAME2, "Zorblax", "Brenwick", SECRET_TEXT, OTHER_TEXT, "fractions", "private noise", "77001"):
                self.assertNotIn(word, seen)

    def test_hostile_edu_code_never_prints_a_text(self):
        self.scenarios[NAME + "|2031-03-03"] = {"exit": 2, "stdout": json.dumps({"status": NAME}) + "\n"}
        path = self.record(call_lines(1, NAME, "2031-03-03", ts_answer("77001", ts_fields({"progress": "a"}))))
        done = self.run_replay("--edu", self.edu, path)
        self.assertNotIn("Zorblax", done.stdout)
        self.assertIn("call 1 refused edu_answer_invalid", done.stdout)

    def test_json_output(self):
        path = self.record(self.build_session())
        done = self.run_replay("--edu", self.edu, "--json", path)
        self.assertEqual(done.returncode, 0)
        doc = json.loads(done.stdout)
        self.assertEqual(doc["total"], {"calls": 8, "equal": 2, "differs": 1, "not_covered": 1,
                                        "refused": 1, "ts_unavailable": 3})
        self.assertEqual(doc["calls"][2], {"call": 3, "outcome": "differs",
                                            "detail": ["homework", "daily_test_assessment"]})
        self.assertEqual(doc["calls"][4]["detail"], ["lesson_student_not_found"])

    def test_two_rows_pick_the_record_of_the_tool(self):
        values = {"progress": "x", "homework": "y"}
        self.scenarios[NAME + "|2031-03-03"] = edu_ok("77001", [
            row("77001", NAME, "3", "50001", "40", "Y", "other", "other", "", "", ""),
            row("77001", NAME, "4", "50009", "41", "Y", "x", "y", "", "", "")])
        path = self.record(call_lines(1, NAME, "2031-03-03", ts_answer("77001", ts_fields(values, course="4", membership="50009"))))
        done = self.run_replay("--edu", self.edu, path)
        self.assertIn("call 1 equal\n", done.stdout)

    def test_two_files_number_calls_in_order(self):
        self.scenarios[NAME + "|2031-03-03"] = edu_ok("77001", [
            row("77001", NAME, "3", "50001", "41", "Y", "a", "b", "", "", "")])
        values = {"progress": "a", "homework": "b"}
        one = self.record(call_lines(1, NAME, "2031-03-03", ts_answer("77001", ts_fields(values))), "a.jsonl")
        two = self.record(call_lines(1, NAME, "2031-03-03", ts_answer("77001", ts_fields(values))), "b.jsonl")
        done = self.run_replay("--edu", self.edu, one, two)
        self.assertEqual(done.stdout.splitlines()[:2], ["call 1 equal", "call 2 equal"])
        self.assertEqual(len(self.edu_calls()), 1)

    def test_usage_errors_exit_two(self):
        path = self.record(["{}"])
        for args in ([], ["--edu", self.edu], [path], ["--edu", os.path.join(self.dir.name, "none"), path],
                     ["--edu", self.edu, "--bogus", path], ["--edu", self.edu, os.path.join(self.dir.name, "none.jsonl")],
                     ["--edu", self.edu, "--edu", self.edu, path]):
            done = self.run_replay(*args)
            self.assertEqual(done.returncode, 2, args)
            self.assertEqual(done.stdout, "")
        self.assertEqual(self.edu_calls(), [])

    def test_edu_that_cannot_run_is_refused_not_fatal(self):
        bad = os.path.join(self.dir.name, "bad")
        with open(bad, "w") as f:
            f.write("#!/nonexistent/interpreter\n")
        os.chmod(bad, 0o755)
        path = self.record(call_lines(1, NAME, "2031-03-03", ts_answer("77001", ts_fields({"progress": "a"}))))
        done = self.run_replay("--edu", bad, path)
        self.assertEqual(done.returncode, 0)
        self.assertIn("call 1 refused edu_not_runnable", done.stdout)


if __name__ == "__main__":
    unittest.main()
