"""Offline regression checks; these never launch a model session."""
import unittest
import json
import run
import rescore
import tempfile
from pathlib import Path


class HarnessTests(unittest.TestCase):
    @staticmethod
    def proof(events, allowed, **kwargs):
        return run.isolation(events, allowed, hook_events_requested=True, **kwargs)

    def test_modes_and_flags(self):
        for mode in ("isolated", "installed"):
            command = run.command("claude", "haiku", "mcp.json", "tools", mode)
            self.assertIn("--no-session-persistence", command)
            self.assertIn("--strict-mcp-config", command)
            self.assertEqual(command[command.index("--output-format") + 1], "stream-json")
            self.assertIn("--verbose", command)
            self.assertIn("--include-hook-events", command)
            self.assertNotIn("--bare", command)
            if mode == "isolated":
                for flag in ("--setting-sources", "--tools"):
                    self.assertEqual(command[command.index(flag) + 1], "")

    def test_exact_isolated_command(self):
        self.assertEqual(run.command("claude", "haiku", "mcp.json", "tools", "isolated"), [
            "claude", "--print", "--model", "haiku", "--mcp-config", "mcp.json",
            "--strict-mcp-config", "--allowedTools", "tools", "--output-format", "stream-json",
            "--verbose", "--include-hook-events", "--no-session-persistence", "--setting-sources", "", "--tools", "",
            "--disable-slash-commands", "--system-prompt",
            "You are an assistant of a teacher. Use the tools you are given."])

    def test_exact_installed_command(self):
        self.assertEqual(run.command("claude", "haiku", "mcp.json", "tools", "installed"), [
            "claude", "--print", "--model", "haiku", "--mcp-config", "mcp.json",
            "--strict-mcp-config", "--allowedTools", "tools", "--output-format", "stream-json",
            "--verbose", "--include-hook-events", "--no-session-persistence", "--disallowedTools",
            "Bash,Read,Grep,Glob,Edit,Write,NotebookEdit,Agent,WebFetch,WebSearch"])

    def test_builtin_plugins_and_six_agents_allowed(self):
        events = self.isolated_events()
        events[0]["plugins"] = [{"name": "cli-plugin", "path": "builtin"}]
        events[1]["message"]["usage"] = {"input_tokens": 2,
            "cache_read_input_tokens": 0, "cache_creation_input_tokens": 4519}
        result = self.proof(events, ["mcp__academy__academy_lesson_prepare"])
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["first_request_tokens"], 4521)
        for plugins in [[{"path": "/synthetic/plugin"}], [{"name": "missing-path"}], ["invalid-entry"]]:
            events[0]["plugins"] = plugins
            self.assertIn("plugins_loaded", self.proof(events, ["mcp__academy__academy_lesson_prepare"])["reasons"])

    @staticmethod
    def isolated_events():
        return [
            {"type": "system", "subtype": "init", "tools": ["mcp__academy__academy_lesson_prepare"],
             "slash_commands": [], "terminal_slash_commands": [], "skills": [], "plugins": [],
             "agents": ["claude", "Explore", "general-purpose", "Plan", "statusline-setup", "web-fetch"],
             "memory_paths": [], "mcp_servers": [{"name": "academy", "status": "connected"}]},
            {"type": "assistant", "message": {"usage": {
                "input_tokens": 100, "cache_read_input_tokens": 200,
                "cache_creation_input_tokens": 300}}},
            # A second request must not be added to the first request's count.
            {"type": "assistant", "message": {"usage": {
                "input_tokens": 5000, "cache_read_input_tokens": 5000,
                "cache_creation_input_tokens": 5000}}},
        ]

    def test_isolation_pass(self):
        result = self.proof(self.isolated_events(), ["mcp__academy__academy_lesson_prepare"])
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["first_request_tokens"], 600)
        self.assertEqual(result["init"]["agents"], list(run.DEFAULT_CLI_AGENTS))
        self.assertIn("terminal_slash_commands", result["checked_init_fields"])

    def test_memory_path_is_metadata_not_a_loaded_file(self):
        events = self.isolated_events()
        events[0]["memory_paths"] = {"auto": "/synthetic/new-cwd/memory/"}
        result = self.proof(events, ["mcp__academy__academy_lesson_prepare"])
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["init"]["memory_paths"], events[0]["memory_paths"])
        self.assertIn("memory_paths", result["observed_init_fields"])
        self.assertNotIn("memory_paths", result["checked_init_fields"])

    def test_isolation_failures(self):
        for field, value, reason in [
            ("tools", ["Read"], "unexpected_tools"),
            ("slash_commands", ["custom"], "slash_commands_loaded"),
            ("terminal_slash_commands", ["custom"], "terminal_slash_commands_loaded"),
            ("skills", ["custom"], "skills_loaded"),
            ("plugins", [{"name": "builtin-plugin"}], "plugins_loaded"),
            ("agents", ["custom-agent"], "unexpected_agents"),
            ("mcp_servers", [{"name": "other"}], "unexpected_mcp_servers"),
        ]:
            with self.subTest(field=field):
                events = self.isolated_events()
                events[0][field] = value
                result = self.proof(events, ["mcp__academy__academy_lesson_prepare"])
                self.assertEqual(result["status"], "FAIL")
                self.assertIn(reason, result["reasons"])
        for field in ("tools", "slash_commands", "terminal_slash_commands", "skills", "plugins",
                      "agents", "mcp_servers"):
            with self.subTest(invalid_field=field):
                events = self.isolated_events()
                events[0][field] = None
                self.assertIn(f"{field}_invalid", self.proof(events, [])["reasons"])
        self.assertIn("malformed_stream", self.proof(self.isolated_events(),
                      ["mcp__academy__academy_lesson_prepare"], malformed=1)["reasons"])
        for event in [{"type": "system", "subtype": "hook_started"}, {"type": "hook_response"}]:
            result = self.proof(self.isolated_events() + [event], ["mcp__academy__academy_lesson_prepare"])
            self.assertIn("hook_events_present", result["reasons"])
        events = self.isolated_events()
        events[1]["message"]["usage"]["input_tokens"] = 5500
        result = self.proof(events, ["mcp__academy__academy_lesson_prepare"])
        self.assertEqual(result["first_request_tokens"], 6000)
        self.assertIn("first_request_tokens_not_under_6000", result["reasons"])
        events[1]["message"]["usage"]["input_tokens"] = 5499
        self.assertEqual(self.proof(events, ["mcp__academy__academy_lesson_prepare"])["status"], "pass")
        self.assertIn("init_missing", self.proof([], [])["reasons"])
        self.assertIn("first_request_usage_missing", self.proof(events[:1], [])["reasons"])
        events = self.isolated_events()
        del events[0]["tools"]
        self.assertIn("tools_missing", self.proof(events, [])["reasons"])
        events = self.isolated_events()
        events.insert(1, dict(events[0]))
        self.assertIn("multiple_init_events", self.proof(events, [])["reasons"])
        events = self.isolated_events()
        events[1]["message"]["usage"] = {"input_tokens": 1}
        self.assertIn("first_request_usage_missing", self.proof(events, [])["reasons"])

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

    @staticmethod
    def academy_exchange(text):
        return [
            {"type":"assistant", "message":{"content":[{"type":"tool_use", "id":"academy-1",
                "name":"mcp__academy__academy_lesson_prepare", "input":{}}]}},
            {"type":"user", "message":{"content":[{"type":"tool_result", "tool_use_id":"academy-1", "content":text}]}},
        ]

    def test_verbatim_display_in_both_received_forms(self):
        for display in ("display field", ""):
            for content, form in [(display, "display_text"),
                                  (json.dumps({"display":display, "status":"ok"}), "envelope")]:
                with self.subTest(form=form, display=display):
                    events = self.academy_exchange(content) + [{"type":"result", "result":display}]
                    result = run.measure(events)
                    self.assertEqual(result["received_form"], form)
                    self.assertEqual(result["display"], display)
                    self.assertEqual(result["verbatim"], "pass")
                    self.assertEqual(result["extra_chars"], 0)

    def test_later_non_academy_result_is_ignored(self):
        events = self.academy_exchange("display field") + [
            {"type":"assistant", "message":{"content":[{"type":"tool_use", "id":"other",
                "name":"Skill", "input":{}}]}},
            {"type":"user", "message":{"content":[{"type":"tool_result", "tool_use_id":"other", "content":"irrelevant text"}]}},
            {"type":"result", "result":"prefix display field suffix"},
        ]
        result = run.measure(events)
        self.assertEqual(result["display"], "display field")
        self.assertEqual(result["verbatim"], "pass")
        self.assertEqual(result["extra_chars"], len("prefix  suffix"))

    def test_no_academy_result_is_unknown(self):
        for events in [[], [{"type":"assistant", "message":{"content":[
            {"type":"tool_use", "id":"missing", "name":"Skill", "input":{}}]}},
            {"type":"user", "message":{"content":[
            {"type":"tool_result", "tool_use_id":"missing", "content":"answer"}]}},
            {"type":"result", "result":"answer"}]]:
            result = run.measure(events)
            self.assertEqual(result["received_form"], "unknown")
            self.assertEqual(result["verbatim"], "unknown")
            self.assertEqual(result["extra_chars"], "unknown")

    def test_last_academy_result_and_nonverbatim(self):
        events = self.academy_exchange("first") + [
            {"type":"assistant", "message":{"content":[{"type":"tool_use", "id":"academy-2",
                "name":"mcp__academy__academy_unit_status", "input":{}}]}},
            {"type":"user", "message":{"content":[{"type":"tool_result", "tool_use_id":"academy-2", "content":"last"}]}},
            {"type":"result", "result":"rewritten"},
        ]
        result = run.measure(events)
        self.assertEqual(result["display"], "last")
        self.assertEqual(result["verbatim"], "FAIL")
        self.assertEqual(result["extra_chars"], len("rewritten"))

    def test_non_display_json_remains_received_text(self):
        for text in [json.dumps({"display": 1}), json.dumps({"status": "ok"}), json.dumps(["text"])]:
            result = run.measure(self.academy_exchange(text) + [{"type":"result", "result":text}])
            self.assertEqual(result["received_form"], "display_text")
            self.assertEqual(result["verbatim"], "pass")

    def test_rescore_exports_only_row_values_and_preserves_evidence(self):
        secret = "synthetic confidential sentinel"
        events = self.isolated_events() + self.academy_exchange(json.dumps({"display":"finished"})) + [
            {"type":"assistant", "message":{"content":[{"type":"tool_use", "id":"skill", "name":"Skill", "input":{"value":secret}}]}},
            {"type":"user", "message":{"content":[{"type":"tool_result", "tool_use_id":"skill", "content":secret}]}},
            {"type":"result", "result":"finished"},
        ]
        with tempfile.TemporaryDirectory(prefix="teacher-rescore-test-") as directory:
            path = Path(directory)
            metadata = json.dumps({"mode":"isolated", "request":"Q01", "model":"haiku", "answer":secret})
            stream = "\n".join(json.dumps(event) for event in events)
            (path / "measurement.json").write_text(metadata)
            (path / "claude.jsonl").write_text(stream)
            row = rescore.score_saved(path)
            self.assertEqual(set(row), {"mode","request","model","received_form","verbatim","extra_chars","isolation"})
            self.assertEqual(row["received_form"], "envelope")
            self.assertEqual(row["verbatim"], "pass")
            self.assertEqual(row["extra_chars"], 0)
            self.assertEqual(row["isolation"], "pass (hooks unknown)")
            self.assertNotIn(secret, json.dumps(row))
            self.assertEqual((path / "measurement.json").read_text(), metadata)
            self.assertEqual((path / "claude.jsonl").read_text(), stream)

    def test_malformed_stream_gives_unknown_reference_checks(self):
        events = self.academy_exchange("display") + [{"type":"result", "result":"display"}]
        raw = "not json\n" + "\n".join(json.dumps(event) for event in events)
        result = run.score_stream(raw, {}, ["mcp__academy__academy_lesson_prepare"], True)
        self.assertEqual(result["received_form"], "unknown")
        self.assertEqual(result["verbatim"], "unknown")
        self.assertEqual(result["extra_chars"], "unknown")
        self.assertIn("malformed_stream", result["isolation"]["reasons"])

    def test_hook_visibility(self):
        events = self.isolated_events()
        allowed = ["mcp__academy__academy_lesson_prepare"]
        old = run.isolation(events, allowed, hook_events_requested=False)
        self.assertEqual(old["hook_condition"], "unknown")
        self.assertEqual(old["status"], "pass (hooks unknown)")
        current = run.isolation(events, allowed, hook_events_requested=True)
        self.assertEqual(current["hook_condition"], "pass")
        self.assertEqual(current["status"], "pass")
        events += [{"type":"system", "subtype":"hook_response"}]
        bad = run.isolation(events, allowed, hook_events_requested=True)
        self.assertEqual(bad["hook_condition"], "FAIL")
        self.assertEqual(bad["status"], "FAIL")

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
