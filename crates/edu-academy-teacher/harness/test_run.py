"""Offline regression checks; these never launch a model session."""
import unittest
import json
import run
import rescore
import test_fidelity
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
            "--verbose", "--include-hook-events", "--no-session-persistence", "--tools", "Skill"])

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
        for display in ("display field", "", " \t\n"):
            for content, form in [(display, "display_text"),
                                  (json.dumps({"display":display, "status":"ok"}), "envelope")]:
                with self.subTest(form=form, display=display):
                    events = self.academy_exchange(content) + [{"type":"result", "result":display}]
                    result = run.measure(events)
                    self.assertEqual(result["received_form"], form)
                    self.assertEqual(result["display"], display)
                    self.assertEqual(result["verbatim"], "pass" if display.strip() else "unknown")
                    self.assertEqual(result["extra_chars"], 0 if display.strip() else "unknown")

    def test_installed_isolation_reduced_fail_closed(self):
        allowed = ["mcp__academy__academy_lesson_prepare"]
        def score(events, prefix=""):
            return run.score_stream(prefix + "\n".join(json.dumps(e) for e in events), {}, allowed, True, "installed")["isolation"]
        events = self.isolated_events()
        events[0]["tools"].append("Skill")
        events[0]["skills"] = ["installed-skill"]
        events += [{"type":"system", "subtype":"hook_response"}]
        good = score(events)
        self.assertEqual(good["status"], "not_applicable")
        self.assertEqual(good["reasons"], [])
        for tools, expected in [(["Read"], ["unexpected_tools"]), (None, ["tools_invalid"])]:
            events[0]["tools"] = tools
            bad = score(events)
            self.assertEqual(bad["status"], "FAIL")
            self.assertEqual(bad["reasons"], expected)
        del events[0]["tools"]
        self.assertEqual(score(events)["reasons"], ["tools_missing"])
        self.assertEqual(score([])["reasons"], ["init_missing", "tools_missing"])
        events = self.isolated_events()
        self.assertEqual(score(events, "broken\n")["reasons"], ["malformed_stream"])
        self.assertEqual(score(events + [events[0]])["reasons"], ["multiple_init_events"])

    def test_delivery_compares_each_matching_call_in_utf8_bytes(self):
        events = self.academy_exchange("질문") + [{"type":"result", "result":"질문"}]
        raw = "\n".join(json.dumps(e) for e in events)
        log = [{"tool":"academy_lesson_prepare", "display_bytes":len("질문".encode("utf-8"))}]
        scored = run.score_stream(raw, {}, [], call_log=log)
        self.assertEqual(scored["received_text_bytes"], 6)
        self.assertEqual(scored["delivery"], "same")
        log[0]["display_bytes"] += 1
        self.assertEqual(run.score_stream(raw, {}, [], call_log=log)["delivery"], "truncated_or_changed")
        log[0]["tool"] = "other"
        self.assertEqual(run.score_stream(raw, {}, [], call_log=log)["delivery"], "unknown")
        self.assertEqual(run.score_stream(raw, {}, [])["delivery"], "unknown")
        calls = [{"name":"mcp__academy__academy_lesson_prepare", "received_text_bytes":n} for n in (6, 3)]
        log = [{"tool":"academy_lesson_prepare", "display_bytes":n} for n in (6, 3)]
        self.assertEqual(run.delivery(calls, log), "same")
        log[0]["display_bytes"] = 3
        self.assertEqual(run.delivery(calls, log), "truncated_or_changed")
        self.assertEqual(run.delivery(calls, log[:1]), "unknown")

    def test_q03_requires_labels_and_one_academy_call(self):
        spec = json.loads((run.HERE / "checks.json").read_text())["Q03"]
        text = "어느 반인가요?\n- 가상반A: 반 전체 준비\n- 가상반B: 반 전체 준비\n- 가상반C: 반 전체 준비"
        events = self.academy_exchange(text) + [{"type":"result", "result":text}]
        def score(events):
            return run.score_stream("\n".join(json.dumps(e) for e in events), spec, [])
        good = score(events)
        self.assertEqual(good["verbatim"], "pass")
        self.assertEqual(good["shown_unchanged"], "pass")
        self.assertEqual(good["facts_kept"], "pass")
        self.assertEqual(set(good["checks"].values()), {"pass"})
        events[-1]["result"] = "어느 반인가요?"
        self.assertEqual(score(events)["checks"]["option:가상반A"], "FAIL")
        self.assertEqual(score(events)["facts_kept"], "FAIL")
        self.assertEqual(score(events + self.academy_exchange(text))["checks"]["academy_calls_before_answer"], "FAIL")

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

    def test_result_columns_share_live_and_offline_scoring(self):
        text = test_fidelity.AcceptanceTests.TEXT
        answer = text.replace('읽은 원천 Source listing\n', '')
        events = self.academy_exchange(text) + [{"type": "result", "result": answer}]
        raw = "\n".join(json.dumps(event) for event in events)
        spec = json.loads((run.HERE / 'checks.json').read_text())["Q01"]
        live = run.score_stream(raw, spec, [])
        self.assertEqual(live['shown_unchanged'], 'FAIL')
        self.assertEqual(live['facts_kept'], 'pass')
        self.assertEqual(live['source_footer'], {'total':1,'kept':0})
        self.assertEqual(live['closing_statement'], {'total':1,'kept':1})
        with tempfile.TemporaryDirectory(prefix='teacher-fidelity-test-') as directory:
            path = Path(directory)
            (path / 'measurement.json').write_text(json.dumps({'mode':'installed', 'request':'Q01', 'model':'haiku'}))
            (path / 'claude.jsonl').write_text(raw)
            offline = rescore.score_saved(path)
        for key in ('shown_unchanged', 'facts_kept', 'source_footer', 'closing_statement'):
            self.assertEqual(offline[key], live[key])
        for malformed in ('not json\n' + raw, ''):
            scored = run.score_stream(malformed, spec, [])
            self.assertEqual(scored['facts_kept'], 'unknown')
            self.assertEqual(scored['shown_unchanged'], 'unknown')

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
            self.assertEqual(set(row), {"mode","request","model","received_form","shown_unchanged","facts_kept","source_footer","closing_statement","failed_rules","extra_chars","isolation","delivery"})
            self.assertEqual(row["received_form"], "envelope")
            self.assertEqual(row["shown_unchanged"], "pass")
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
        self.assertEqual(result["shown_unchanged"], "unknown")
        self.assertEqual(result["facts_kept"], "unknown")
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
