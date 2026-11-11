"""Offline regression checks; these never launch a model session."""
import unittest
import json
import run


class HarnessTests(unittest.TestCase):
    def test_modes_and_flags(self):
        for mode in ("isolated", "installed"):
            command = run.command("claude", "haiku", "mcp.json", "tools", mode)
            self.assertIn("--no-session-persistence", command)
            self.assertIn("--strict-mcp-config", command)
            self.assertEqual(command[command.index("--output-format") + 1], "stream-json")
            self.assertIn("--verbose", command)
            self.assertNotIn("--bare", command)
            if mode == "isolated":
                for flag in ("--setting-sources", "--tools"):
                    self.assertEqual(command[command.index(flag) + 1], "")

    def test_exact_isolated_command(self):
        self.assertEqual(run.command("claude", "haiku", "mcp.json", "tools", "isolated"), [
            "claude", "--print", "--model", "haiku", "--mcp-config", "mcp.json",
            "--strict-mcp-config", "--allowedTools", "tools", "--output-format", "stream-json",
            "--verbose", "--no-session-persistence", "--setting-sources", "", "--tools", "",
            "--disable-slash-commands", "--system-prompt",
            "You are an assistant of a teacher. Use the tools you are given."])

    def test_exact_installed_command(self):
        self.assertEqual(run.command("claude", "haiku", "mcp.json", "tools", "installed"), [
            "claude", "--print", "--model", "haiku", "--mcp-config", "mcp.json",
            "--strict-mcp-config", "--allowedTools", "tools", "--output-format", "stream-json",
            "--verbose", "--no-session-persistence", "--disallowedTools",
            "Bash,Read,Grep,Glob,Edit,Write,NotebookEdit,Agent,WebFetch,WebSearch"])

    def test_builtin_plugins_and_six_agents_allowed(self):
        events = self.isolated_events()
        events[0]["plugins"] = [{"name": "cli-plugin", "path": "builtin"}]
        events[1]["message"]["usage"] = {"input_tokens": 2,
            "cache_read_input_tokens": 0, "cache_creation_input_tokens": 4519}
        result = run.isolation(events, ["mcp__academy__academy_lesson_prepare"])
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["first_request_tokens"], 4521)
        for plugins in [[{"path": "/synthetic/plugin"}], [{"name": "missing-path"}], ["invalid-entry"]]:
            events[0]["plugins"] = plugins
            self.assertIn("plugins_loaded", run.isolation(events, ["mcp__academy__academy_lesson_prepare"])["reasons"])

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
        result = run.isolation(self.isolated_events(), ["mcp__academy__academy_lesson_prepare"])
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["first_request_tokens"], 600)
        self.assertEqual(result["init"]["agents"], list(run.DEFAULT_CLI_AGENTS))
        self.assertIn("terminal_slash_commands", result["checked_init_fields"])

    def test_memory_path_is_metadata_not_a_loaded_file(self):
        events = self.isolated_events()
        events[0]["memory_paths"] = {"auto": "/synthetic/new-cwd/memory/"}
        result = run.isolation(events, ["mcp__academy__academy_lesson_prepare"])
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
                result = run.isolation(events, ["mcp__academy__academy_lesson_prepare"])
                self.assertEqual(result["status"], "FAIL")
                self.assertIn(reason, result["reasons"])
        for field in ("tools", "slash_commands", "terminal_slash_commands", "skills", "plugins",
                      "agents", "mcp_servers"):
            with self.subTest(invalid_field=field):
                events = self.isolated_events()
                events[0][field] = None
                self.assertIn(f"{field}_invalid", run.isolation(events, [])["reasons"])
        self.assertIn("malformed_stream", run.isolation(self.isolated_events(),
                      ["mcp__academy__academy_lesson_prepare"], malformed=1)["reasons"])
        for event in [{"type": "system", "subtype": "hook_started"}, {"type": "hook_response"}]:
            result = run.isolation(self.isolated_events() + [event], ["mcp__academy__academy_lesson_prepare"])
            self.assertIn("hook_events_present", result["reasons"])
        events = self.isolated_events()
        events[1]["message"]["usage"]["input_tokens"] = 5500
        result = run.isolation(events, ["mcp__academy__academy_lesson_prepare"])
        self.assertEqual(result["first_request_tokens"], 6000)
        self.assertIn("first_request_tokens_not_under_6000", result["reasons"])
        events[1]["message"]["usage"]["input_tokens"] = 5499
        self.assertEqual(run.isolation(events, ["mcp__academy__academy_lesson_prepare"])["status"], "pass")
        self.assertIn("init_missing", run.isolation([], [])["reasons"])
        self.assertIn("first_request_usage_missing", run.isolation(events[:1], [])["reasons"])
        events = self.isolated_events()
        del events[0]["tools"]
        self.assertIn("tools_missing", run.isolation(events, [])["reasons"])
        events = self.isolated_events()
        events.insert(1, dict(events[0]))
        self.assertIn("multiple_init_events", run.isolation(events, [])["reasons"])
        events = self.isolated_events()
        events[1]["message"]["usage"] = {"input_tokens": 1}
        self.assertIn("first_request_usage_missing", run.isolation(events, [])["reasons"])

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

    def test_received_envelope_text_is_preserved(self):
        # Claude can forward structuredContent as JSON text instead of the
        # server's content[0].text. Measure the actual received representation.
        text = json.dumps({"display": "display field", "status": "ok"})
        events = [{"type": "user", "message": {"content": [
            {"type": "tool_result", "content": text}]}},
            {"type": "result", "result": "display field"}]
        result = run.measure(events)
        self.assertEqual(result["display"], text)
        self.assertEqual(result["verbatim"], "FAIL")

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
