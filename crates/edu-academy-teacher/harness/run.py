#!/usr/bin/env python3
"""One measured run: Q01|Q02 x model, against the built teacher binary only."""
import argparse, json, os, subprocess, sys, tempfile, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
CRATE = HERE.parent

def norm(text):
    return " ".join((text or "").split())

def dig(data, dotted):
    cur = data
    for part in dotted.split("."):
        cur = cur[int(part)] if isinstance(cur, list) else cur[part]
    return cur

def probe(binary, scenario, arguments):
    lines = [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize",
         "params": {"protocolVersion": "2025-06-18", "capabilities": {}}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
         "params": {"name": "academy_lesson_prepare", "arguments": arguments}},
    ]
    out = subprocess.run([str(binary), "--scenario", str(scenario), "--today", "2026-11-11"],
                         input="\n".join(json.dumps(x) for x in lines) + "\n",
                         capture_output=True, text=True, timeout=60)
    reply = json.loads(out.stdout.strip().splitlines()[-1])
    return reply["result"]["structuredContent"]

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("request", choices=["Q01", "Q02"])
    ap.add_argument("model")
    ap.add_argument("--binary", default=os.environ.get("EDU_ACADEMY_TEACHER_BINARY", ""))
    ap.add_argument("--scenario", default=str(CRATE / "tests/fixtures/scenario-v1.json"))
    ap.add_argument("--claude", default=os.environ.get("CLAUDE_BIN", "claude"))
    args = ap.parse_args()
    spec = json.loads((HERE / "checks.json").read_text(encoding="utf-8"))[args.request]
    binary = Path(args.binary) if args.binary else Path(
        "/home/palantirkc/work/ws-replan/wt/e09d-target/debug/edu-academy-teacher")
    if not binary.exists():
        binary = CRATE.parent.parent / "target/debug/edu-academy-teacher"
    binary, scenario = binary.resolve(), Path(args.scenario).resolve()
    tools = json.loads((CRATE / "tests/fixtures/tools-list.json").read_text(encoding="utf-8"))["tools"]
    allowed = ",".join(f"mcp__academy__{t['name']}" for t in tools)
    envelope = probe(binary, scenario, spec["tool_arguments"])
    display = (HERE / spec["display_fixture"]).read_text(encoding="utf-8")
    root = Path(tempfile.mkdtemp(prefix=f"p8-{args.request}-{args.model}-"))
    run, cwd = root / "run", root / "cwd"
    run.mkdir(); cwd.mkdir()
    log_path = run / "run-log.jsonl"
    config = {"mcpServers": {"academy": {"command": str(binary), "args": [
        "--scenario", str(scenario), "--today", "2026-11-11", "--run-log", str(log_path)]}}}
    config_path = run / "mcp.json"
    config_path.write_text(json.dumps(config), encoding="utf-8")
    started = time.time()
    try:
        proc = subprocess.run(
            [args.claude, "--print", "--model", args.model, "--mcp-config", str(config_path),
             "--strict-mcp-config", "--allowedTools", allowed, "--output-format", "json"],
            input=spec["request_text"], capture_output=True, text=True, cwd=cwd, timeout=600)
        raw, stderr = proc.stdout, proc.stderr
    except Exception as exc:  # a failed run is a row, not a harness crash
        raw, stderr = "", f"{type(exc).__name__}: {exc}"
    wall = time.time() - started
    try:
        payload = json.loads(raw) if raw.strip() else {}
    except json.JSONDecodeError:
        payload = {}
    answer = payload.get("result") or ""
    (run / "answer.txt").write_text(answer, encoding="utf-8")
    (run / "claude.json").write_text(raw or "{}", encoding="utf-8")
    if stderr:
        (run / "claude.stderr").write_text(stderr, encoding="utf-8")
    rows = []
    if log_path.exists():
        rows = [json.loads(x) for x in log_path.read_text(encoding="utf-8").splitlines() if x.strip()]
    codes = ",".join(r["code"] if r["code"] else "-" for r in rows) or "none"
    checks, failed = [], []
    def check(name, ok, detail=""):
        checks.append(f"{name}={'pass' if ok else 'FAIL'}")
        if not ok:
            failed.append(f"{name}: {detail or 'see answer.txt'}")
    for dotted, want in spec.get("data_equals", {}).items():
        try:
            got = dig(envelope.get("data", {}), dotted)
        except Exception:
            got = None
        check(f"data.{dotted}", got == want, f"got {got!r}")
    for bad in spec.get("answer_forbidden", []):
        check(f"forbidden:{bad}", bad not in answer, "present in answer")
    order = spec.get("answer_order", [])
    if order:
        pos = [answer.find(name) for name in order]
        check("roster_order", all(p >= 0 for p in pos) and pos == sorted(pos), f"positions {pos}")
    for i, lc in enumerate(spec.get("line_checks", [])):
        lines = [x for x in answer.splitlines()
                 if all(s in x for s in lc["contains"])]
        if not lines and len(lc["contains"]) > 1:
            # Reformatted answers may put the name on its own line: use the
            # block from that name to the next roster name instead.
            start = answer.find(lc["contains"][0])
            if start >= 0:
                ends = [answer.find(n, start + 1) for n in order
                        if n != lc["contains"][0] and answer.find(n, start + 1) > start]
                window = answer[start:min(ends) if ends else len(answer)]
                lines = [x for x in window.splitlines()
                         if all(s in x for s in lc["contains"][1:])]
        joined = "\n".join(lines)
        ok = bool(lines) and all(s in joined for s in lc["must_contain"]) \
            and not any(s in joined for s in lc["must_not_contain"])
        check(f"line{i}", ok, f"lines={lines!r}")
    verbatim = norm(display) in norm(answer)
    check("display_verbatim", verbatim, "last display not unchanged in answer")
    usage = payload.get("usage") or {}
    tokens = (f"in={usage.get('input_tokens', '?')},out={usage.get('output_tokens', '?')},"
              f"cache_read={usage.get('cache_read_input_tokens', '?')},"
              f"cache_create={usage.get('cache_creation_input_tokens', '?')}")
    cost = payload.get("total_cost_usd", "?")
    print(f"{args.request} | model={args.model} | calls={len(rows)}/{spec['target_calls']} | "
          f"codes={codes} | args_bytes={sum(r['args_bytes'] for r in rows)} | "
          f"display_verbatim={'yes' if verbatim else 'no'} | checks={';'.join(checks)} | "
          f"wall_s={wall:.1f} | tokens={tokens} | cost_usd={cost} | run_dir={run}")
    if failed:
        print("FAILED: " + " | ".join(failed))
    return 0

if __name__ == "__main__":
    sys.exit(main())
