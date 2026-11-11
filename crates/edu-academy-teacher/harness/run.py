#!/usr/bin/env python3
"""Measure one session from its stream, never from a separate server probe."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

import fidelity

HERE = Path(__file__).resolve().parent
CRATE = HERE.parent
ISOLATED_SYSTEM_PROMPT = "You are an assistant of a teacher. Use the tools you are given."
# Accepted CLI builtin agents (TASK-P8-S1b-R1c).
DEFAULT_CLI_AGENTS = ("claude", "Explore", "general-purpose", "Plan", "statusline-setup", "web-fetch")


def norm(text):
    return " ".join(text.split())


def command(cli, model, config, allowed, mode):
    args = [cli, "--print", "--model", model, "--mcp-config", str(config),
            "--strict-mcp-config", "--allowedTools", allowed,
            "--output-format", "stream-json", "--verbose", "--include-hook-events", "--no-session-persistence"]
    if mode == "isolated":
        args += ["--setting-sources", "", "--tools", "",
                 "--disable-slash-commands", "--system-prompt", ISOLATED_SYSTEM_PROMPT]
    else:
        args += ["--tools", "Skill"]
    return args


def received_text(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list) and content and all(
            isinstance(b, dict) and b.get("type") == "text" and isinstance(b.get("text"), str)
            for b in content):
        return "\n".join(b["text"] for b in content)
    return None


def measure(events):
    calls, display, payload = [], None, {}
    call_names, call_details, received_form = {}, {}, "unknown"
    received_bytes = None
    for event in events:
        if event.get("type") in ("assistant", "user"):
            blocks = event.get("message", {}).get("content", [])
            if not isinstance(blocks, list):
                continue
            for block in blocks:
                if not isinstance(block, dict):
                    continue
                if block.get("type") == "tool_use":
                    if isinstance(block.get("id"), str):
                        call_names[block["id"]] = block.get("name") or ""
                    arguments = block.get("input")
                    encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"))
                    calls.append({"name": block.get("name"), "arguments": encoded,
                                  "args_bytes": len(encoded.encode("utf-8")) if "input" in block else None})
                    if isinstance(block.get("id"), str):
                        call_details[block["id"]] = calls[-1]
                elif block.get("type") == "tool_result" and str(
                        call_names.get(block.get("tool_use_id"), "")).startswith("mcp__academy__"):
                    display = received_text(block.get("content"))
                    received_bytes = len(display.encode("utf-8")) if display is not None else None
                    call_details[block["tool_use_id"]]["received_text_bytes"] = received_bytes
                    received_form = "unknown"
                    if display is not None:
                        received_form = "display_text"
                        try:
                            envelope = json.loads(display)
                        except json.JSONDecodeError:
                            envelope = None
                        if isinstance(envelope, dict) and isinstance(envelope.get("display"), str):
                            display = envelope["display"]
                            received_form = "envelope"
        elif event.get("type") == "result":
            payload = event
    answer = payload.get("result")
    if not isinstance(answer, str) or payload.get("is_error"):
        answer = None
    verbatim, extra = "unknown", "unknown"
    if display is not None and norm(display) and answer is not None:
        shown, final = norm(display), norm(answer)
        present = shown in final
        verbatim = "pass" if present else "FAIL"
        extra = len(final) - len(shown) if present else len(final)
    return {"calls": calls, "display": display, "received_form": received_form, "answer": answer,
            "verbatim": verbatim, "extra_chars": extra, "received_text_bytes": received_bytes,
            "usage": payload.get("usage") or {}, "cost_usd": payload.get("total_cost_usd", "unknown"),
            "result_received": bool(payload), "is_error": payload.get("is_error", "unknown")}


def isolation(events, allowed_tools, malformed=0, hook_events_requested=False, mode="isolated"):
    """Fail closed on missing evidence, allowing only CLI builtin extensions."""
    reasons = []
    inits = [e for e in events if e.get("type") == "system" and e.get("subtype") == "init"]
    if not inits:
        reasons.append("init_missing")
    elif len(inits) != 1:
        reasons.append("multiple_init_events")
    init = inits[0] if inits else {}
    fields = ("tools", "slash_commands", "terminal_slash_commands", "skills", "plugins",
              "agents", "memory_paths", "mcp_servers")
    snapshot = {field: init[field] for field in fields if field in init}
    if "tools" not in init:
        reasons.append("tools_missing")
    for field, values in snapshot.items():
        if field == "memory_paths":
            # The CLI reports an auto-memory destination as an object, even
            # for a new empty cwd. A path alone does not prove a file was loaded.
            continue
        if not isinstance(values, list):
            reasons.append(f"{field}_invalid")
            continue
        if field == "tools":
            if any(tool not in allowed_tools and not (mode == "installed" and tool == "Skill") for tool in values):
                reasons.append("unexpected_tools")
        elif field == "plugins":
            if any(not isinstance(plugin, dict) or plugin.get("path") != "builtin" for plugin in values):
                reasons.append("plugins_loaded")
        elif field == "agents":
            if any(agent not in DEFAULT_CLI_AGENTS for agent in values):
                reasons.append("unexpected_agents")
        elif field == "mcp_servers":
            if any(not isinstance(server, dict) or server.get("name") != "academy" for server in values):
                reasons.append("unexpected_mcp_servers")
        elif values:
            reasons.append(f"{field}_loaded")
    hooks = [{"type": e.get("type"), "subtype": e.get("subtype")} for e in events
             if "hook" in str(e.get("type", "")).lower()
             or "hook" in str(e.get("subtype", "")).lower()]
    if hooks:
        reasons.append("hook_events_present")
    hook_condition = "unknown" if not hook_events_requested else "FAIL" if hooks else "pass"
    first = next((e.get("message", {}).get("usage") for e in events if e.get("type") == "assistant"), None)
    keys = ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")
    usage, tokens = {}, None
    if isinstance(first, dict) and all(type(first.get(k)) is int and first[k] >= 0 for k in keys):
        usage = {key: first[key] for key in keys}
        tokens = sum(usage.values())
        if tokens >= 6000:
            reasons.append("first_request_tokens_not_under_6000")
    else:
        reasons.append("first_request_usage_missing")
    if malformed:
        reasons.append("malformed_stream")
    if mode == "installed":
        reduced = {"init_missing", "multiple_init_events", "tools_missing", "tools_invalid",
                   "unexpected_tools", "malformed_stream"}
        reasons = [reason for reason in reasons if reason in reduced]
    status = ("FAIL" if reasons else "not_applicable" if mode == "installed"
              else "pass (hooks unknown)" if hook_condition == "unknown" else "pass")
    return {"status": status, "reasons": reasons, "hook_condition": hook_condition,
            "hook_events_requested": hook_events_requested,
            "checked_init_fields": [field for field in snapshot if field == "tools" or (mode != "installed" and field != "memory_paths")],
            "observed_init_fields": list(snapshot), "init": snapshot,
            "default_cli_agents": list(DEFAULT_CLI_AGENTS), "hook_events": hooks,
            "first_request_usage": usage, "first_request_tokens": tokens}


def answer_checks(spec, answer):
    checks = {}

    def check(name, ok):
        checks[name] = "unknown" if answer is None else "pass" if ok else "FAIL"

    text = answer or ""
    dates = spec.get("answer_date_any", [])
    if dates:
        check("previous_lesson_date", any(date in text for date in dates))
    for bad in spec.get("answer_forbidden", []):
        check(f"forbidden:{bad}", bad not in text)
    order = spec.get("answer_order", [])
    if order:
        positions = [text.find(name) for name in order]
        check("roster_order", all(p >= 0 for p in positions) and positions == sorted(positions))
    for i, lc in enumerate(spec.get("line_checks", [])):
        lines = [x for x in text.splitlines() if all(s in x for s in lc["contains"])]
        if not lines and len(lc["contains"]) > 1:
            start = text.find(lc["contains"][0])
            if start >= 0:
                ends = [text.find(n, start + 1) for n in order
                        if n != lc["contains"][0] and text.find(n, start + 1) > start]
                window = text[start:min(ends) if ends else len(text)]
                lines = [x for x in window.splitlines() if all(s in x for s in lc["contains"][1:])]
        joined = "\n".join(lines)
        check(f"line{i}", bool(lines) and all(s in joined for s in lc["must_contain"])
              and not any(s in joined for s in lc["must_not_contain"]))
    for label in spec.get("answer_labels", []):
        check(f"option:{label}", label in text)
    return checks


def read_call_log(path):
    try:
        rows = [json.loads(line) for line in Path(path).read_text(encoding="utf-8").splitlines()]
    except (OSError, ValueError):
        return None
    return rows if all(isinstance(row, dict) for row in rows) else None


def delivery(calls, call_log):
    """Match server calls and client tool uses in order, including repeated names."""
    academy = [call for call in calls if str(call["name"]).startswith("mcp__academy__")]
    if call_log is None or not academy or len(academy) != len(call_log):
        return "unknown"
    outcomes = []
    for call, row in zip(academy, call_log):
        size, received = row.get("display_bytes"), call.get("received_text_bytes")
        if (call["name"] != "mcp__academy__" + str(row.get("tool"))
                or type(size) is not int or type(received) is not int):
            outcomes.append("unknown")
        else:
            outcomes.append("same" if size == received else "truncated_or_changed")
    return ("truncated_or_changed" if "truncated_or_changed" in outcomes else
            "unknown" if "unknown" in outcomes else "same")


def score_stream(raw, spec, allowed_tools, hook_events_requested=False, mode="isolated", call_log=None):
    """Shared live/offline scoring; never launches a CLI or mutates evidence."""
    events, malformed = [], 0
    for line in raw.splitlines():
        try:
            event = json.loads(line)
            if isinstance(event, dict):
                events.append(event)
            else:
                malformed += 1
        except json.JSONDecodeError:
            malformed += 1
    result = measure(events)
    if malformed:
        result.update(verbatim="unknown", extra_chars="unknown", received_form="unknown")
    result["shown_unchanged"] = result["verbatim"]
    result.update(fidelity.evaluate(result["display"], result["answer"], spec.get("fidelity_policy"))
                  if not malformed else {"facts_kept": "unknown"})
    result["malformed_stream_lines"] = malformed
    result["isolation"] = isolation(events, allowed_tools, malformed, hook_events_requested, mode)
    result["delivery"] = "unknown" if malformed else delivery(result["calls"], call_log)
    result["checks"] = answer_checks(spec, result["answer"])
    result["call_count"] = len(result["calls"]) if result["result_received"] and not malformed else "unknown"
    result["args_bytes"] = sum(c["args_bytes"] for c in result["calls"]) if (
        result["call_count"] != "unknown" and all(c["args_bytes"] is not None for c in result["calls"])) else "unknown"
    academy_count = sum(str(call["name"]).startswith("mcp__academy__") for call in result["calls"])
    result["academy_calls"] = academy_count if result["call_count"] != "unknown" else "unknown"
    if "max_academy_calls" in spec:
        result["checks"]["academy_calls_before_answer"] = ("unknown" if result["academy_calls"] == "unknown"
            else "pass" if academy_count == spec["target_calls"] and academy_count <= spec["max_academy_calls"] else "FAIL")
    return result


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("request", choices=["Q01", "Q02", "Q03"])
    ap.add_argument("model")
    ap.add_argument("--mode", choices=["isolated", "installed"], required=True)
    ap.add_argument("--binary", default=os.environ.get("EDU_ACADEMY_TEACHER_BINARY", ""))
    ap.add_argument("--scenario", default=str(CRATE / "tests/fixtures/scenario-v2.json"))
    ap.add_argument("--claude", default=os.environ.get("CLAUDE_BIN", "claude"))
    args = ap.parse_args()
    spec = json.loads((HERE / "checks.json").read_text(encoding="utf-8"))[args.request]
    binary = Path(args.binary or "/home/palantirkc/work/ws-replan/wt/e09d-target/debug/edu-academy-teacher").resolve()
    scenario = Path(args.scenario).resolve()
    tools = json.loads((CRATE / "tests/fixtures/tools-list.json").read_text(encoding="utf-8"))["tools"]
    allowed = ",".join(f"mcp__academy__{t['name']}" for t in tools)
    root = Path(tempfile.mkdtemp(prefix=f"p8-{args.mode}-{args.request}-{args.model}-"))
    run, cwd = root / "run", root / "cwd"
    run.mkdir()
    cwd.mkdir()
    config = {"mcpServers": {"academy": {"command": str(binary), "args": [
        "--scenario", str(scenario), "--today", "2026-11-11", "--run-log", str(run / "run-log.jsonl")]}}}
    config_path = run / "mcp.json"
    config_path.write_text(json.dumps(config), encoding="utf-8")
    cli_command = command(args.claude, args.model, config_path, allowed, args.mode)
    started = time.monotonic()
    exit_code = None
    try:
        proc = subprocess.run(cli_command,
                              input=spec["request_text"], capture_output=True, text=True, cwd=cwd, timeout=600)
        raw, stderr, exit_code = proc.stdout, proc.stderr, proc.returncode
    except subprocess.TimeoutExpired as exc:
        raw = (exc.stdout or b"")
        if isinstance(raw, bytes):
            raw = raw.decode("utf-8", errors="replace")
        stderr = "CLI timeout after 600 seconds"
    except OSError as exc:
        raw, stderr = "", f"{type(exc).__name__}: CLI could not start"
    wall = time.monotonic() - started
    result = score_stream(raw, spec, allowed.split(","), "--include-hook-events" in cli_command, args.mode, read_call_log(run / "run-log.jsonl"))
    result["command"] = cli_command
    result["system_prompt"] = ISOLATED_SYSTEM_PROMPT if args.mode == "isolated" else None
    result.update({"mode": args.mode, "request": args.request, "model": args.model,
                   "target_calls": spec["target_calls"], "wall_s": round(wall, 3),
                   "exit_code": exit_code, "run_dir": str(run)})
    (run / "answer.txt").write_text(result["answer"] or "", encoding="utf-8")
    (run / "claude.jsonl").write_text(raw, encoding="utf-8")
    (run / "claude.stderr").write_text(stderr, encoding="utf-8")
    (run / "measurement.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    usage = result["usage"]
    tokens = ",".join(f"{key}={usage.get(key, 'unknown')}" for key in (
        "input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"))
    proof = result["isolation"]
    isolation_text = proof["status"] + (":" + ",".join(proof["reasons"]) if proof["reasons"] else "")
    print(f"{args.mode} | {args.request} | {args.model} | calls={result['call_count']}/{spec['target_calls']} | "
          f"args_bytes={result['args_bytes']} | received_text_bytes={result['received_text_bytes']} | delivery={result['delivery']} | isolation={isolation_text} | received_form={result['received_form']} | shown_unchanged={result['shown_unchanged']} | facts_kept={result['facts_kept']} | extra_chars={result['extra_chars']} | "
          f"checks={';'.join(f'{k}={v}' for k, v in result['checks'].items())} | wall_s={wall:.1f} | "
          f"tokens={tokens} | cost_usd={result['cost_usd']} | exit={exit_code} | run_dir={run}")
    if args.mode == "isolated" and "first_request_tokens_not_under_6000" in proof["reasons"]:
        return 3  # Stop the measurement series; inspect the saved stream before any further attempt.
    return 0


if __name__ == "__main__":
    sys.exit(main())
