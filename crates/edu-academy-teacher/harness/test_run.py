"""Offline regression checks; these never launch a model session."""
import unittest
import run


class HarnessTests(unittest.TestCase):
    def test_modes_and_flags(self):
        for mode in ("isolated", "installed"):
            command = run.command("claude", "haiku", "mcp.json", "tools", mode)
            self.assertIn("--no-session-persistence", command)
            self.assertIn("--strict-mcp-config", command)
            self.assertEqual(command[command.index("--output-format") + 1], "stream-json")
            self.assertIn("--verbose", command)
            self.assertEqual("--bare" in command, mode == "isolated")
            if mode == "isolated":
                for flag in ("--setting-sources", "--tools"):
                    self.assertEqual(command[command.index(flag) + 1], "")

    def test_stream_is_the_only_measurement_source(self):
        events = [
            {"type": "assistant", "message": {"content": [
                {"type": "tool_use", "id": "one", "name": "mcp__academy__academy_lesson_prepare", "input": {"student": "synthetic"}}]}},
            {"type": "user", "message": {"content": [
                {"type": "tool_result", "tool_use_id": "one", "content": [{"type": "text", "text": "received display"}]}]}},
            {"type": "result", "result": "prefix received\n display suffix", "usage": {"input_tokens": 7}, "total_cost_usd": 0.1},
        ]
        result = run.measure(events)
        self.assertEqual(result["calls"][0]["args_bytes"], len(b'{"student":"synthetic"}'))
        self.assertEqual(result["display"], "received display")
        self.assertEqual(result["verbatim"], "pass")
        self.assertEqual(result["extra_chars"], len("prefix  suffix"))
        self.assertEqual(result["usage"]["input_tokens"], 7)
        self.assertEqual(result["cost_usd"], 0.1)
        self.assertEqual(run.measure([])["verbatim"], "unknown")
        self.assertEqual(run.measure([])["extra_chars"], "unknown")

    def test_last_received_result_and_nonverbatim(self):
        events = [
            {"type": "user", "message": {"content": [
                {"type": "tool_result", "content": "first"},
                {"type": "tool_result", "content": "last"}]}},
            {"type": "result", "result": "rewritten"},
        ]
        result = run.measure(events)
        self.assertEqual(result["display"], "last")
        self.assertEqual(result["verbatim"], "FAIL")
        self.assertEqual(result["extra_chars"], len("rewritten"))
        self.assertEqual(run.measure(events[:1])["verbatim"], "unknown")

    def test_missing_result_text_is_unknown(self):
        events = [
            {"type": "user", "message": {"content": [{"type": "tool_result", "content": [{"type": "image"}]}]}},
            {"type": "result", "result": "answer"},
        ]
        self.assertEqual(run.measure(events)["verbatim"], "unknown")
        spec = {"answer_date_any": ["2026-11-09"], "answer_forbidden": ["2026-11-04"],
                "answer_order": ["synthetic"], "line_checks": [{"contains": ["video"],
                "must_contain": ["waiting"], "must_not_contain": ["done"]}]}
        self.assertEqual(set(run.answer_checks(spec, None).values()), {"unknown"})
        self.assertIsNone(run.measure([{"type":"result", "is_error":True, "result":"CLI error"}])["answer"])
        self.assertEqual(run.answer_checks({"answer_date_any": ["2026-11-09", "11월 9일"]}, "11월 9일")["previous_lesson_date"], "pass")
        self.assertEqual(run.answer_checks({"answer_date_any": ["2026-11-09", "11월 9일"]}, "11월 4일")["previous_lesson_date"], "FAIL")


if __name__ == "__main__":
    unittest.main()
