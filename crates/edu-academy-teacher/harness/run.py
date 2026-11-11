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

HERE = Path(__file__).resolve().parent
CRATE = HERE.parent


def norm(text):
    return " ".join(text.split())


def command(cli, model, config, allowed, mode):
    args = [cli, "--print", "--model", model, "--mcp-config", str(config),
            "--strict-mcp-config", "--allowedTools", allowed,
            "--output-format", "stream-json", "--verbose", "--no-session-persistence"]
    if mode == "isolated":
        args += ["--bare", "--setting-sources", "", "--tools", "",
                 "--disable-slash-commands"]
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
    for event in events:
        if event.get("type") in ("assistant", "user"):
            blocks = event.get("message", {}).get("content", [])
            if not isinstance(blocks, list):
                continue
            for block in blocks:
                if not isinstance(block, dict):
                    continue
                if block.get("type") == "tool_use":
                    arguments = block.get("input")
                    encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"))
                    calls.append({"name": block.get("name"), "arguments": encoded,
                                  "args_bytes": len(encoded.encode("utf-8")) if "input" in block else None})
                elif block.get("type") == "tool_result":
                    display = received_text(block.get("content"))
        elif event.get("type") == "result":
            payload = event
    answer = payload.get("result")
    if not isinstance(answer, str) or payload.get("is_error"):
        answer = None
    verbatim, extra = "unknown", "unknown"
    if display and answer is not None:
        shown, final = norm(display), norm(answer)
        if shown:
            present = shown in final
            verbatim = "pass" if present else "FAIL"
            extra = len(final) - len(shown) if present else len(final)
    return {"calls": calls, "display": display, "answer": answer,
            "verbatim": verbatim, "extra_chars": extra,
            "usage": payload.get("usage") or {}, "cost_usd": payload.get("total_cost_usd", "unknown"),
            "result_received": bool(payload), "is_error": payload.get("is_error", "unknown")}


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
    return checks


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("request", choices=["Q01", "Q02"])
    ap.add_argument("model")
    ap.add_argument("--mode", choices=["isolated", "installed"], required=True)
    ap.add_argument("--binary", default=os.environ.get("EDU_ACADEMY_TEACHER_BINARY", ""))
    ap.add_argument("--scenario", default=str(CRATE / "tests/fixtures/scenario-v1.json"))
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
    started = time.monotonic()
    exit_code = None
    try:
        proc = subprocess.run(command(args.claude, args.model, config_path, allowed, args.mode),
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
        # A discarded event could have been a later tool result.
        result["verbatim"] = "unknown"
        result["extra_chars"] = "unknown"
    result.update({"mode": args.mode, "request": args.request, "model": args.model,
                   "target_calls": spec["target_calls"], "wall_s": round(wall, 3),
                   "exit_code": exit_code, "malformed_stream_lines": malformed,
                   "checks": answer_checks(spec, result["answer"]), "run_dir": str(run)})
    # Incomplete streams cannot establish that the observed calls are all the calls.
    result["call_count"] = len(result["calls"]) if result["result_received"] and not malformed else "unknown"
    result["args_bytes"] = sum(c["args_bytes"] for c in result["calls"]) if (
        result["call_count"] != "unknown" and all(c["args_bytes"] is not None for c in result["calls"])) else "unknown"
    (run / "answer.txt").write_text(result["answer"] or "", encoding="utf-8")
    (run / "claude.jsonl").write_text(raw, encoding="utf-8")
    (run / "claude.stderr").write_text(stderr, encoding="utf-8")
    (run / "measurement.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    usage = result["usage"]
    tokens = ",".join(f"{key}={usage.get(key, 'unknown')}" for key in (
        "input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"))
    print(f"{args.mode} | {args.request} | {args.model} | calls={result['call_count']}/{spec['target_calls']} | "
          f"args_bytes={result['args_bytes']} | verbatim={result['verbatim']} | extra_chars={result['extra_chars']} | "
          f"checks={';'.join(f'{k}={v}' for k, v in result['checks'].items())} | wall_s={wall:.1f} | "
          f"tokens={tokens} | cost_usd={result['cost_usd']} | exit={exit_code} | run_dir={run}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
