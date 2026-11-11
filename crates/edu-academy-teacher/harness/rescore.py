#!/usr/bin/env python3
"""Offline scoring: only row values leave saved evidence directories."""
import argparse
import json
from pathlib import Path
import sys

import run


def score_saved(path):
    directory = Path(path)
    saved = json.loads((directory / "measurement.json").read_text(encoding="utf-8"))
    mode, request, model = (saved[key] for key in ("mode", "request", "model"))
    if mode not in ("isolated", "installed") or request not in ("Q01", "Q02", "Q03") or model not in ("haiku", "sonnet"):
        raise ValueError("unsupported row identity")
    specs = json.loads((run.HERE / "checks.json").read_text(encoding="utf-8"))
    tools = json.loads((run.CRATE / "tests/fixtures/tools-list.json").read_text(encoding="utf-8"))["tools"]
    allowed = [f"mcp__academy__{tool['name']}" for tool in tools]
    # Missing command metadata cannot establish complete hook visibility.
    command = saved.get("command", [])
    hooks_requested = isinstance(command, list) and "--include-hook-events" in command
    result = run.score_stream((directory / "claude.jsonl").read_text(encoding="utf-8"),
                              specs[request], allowed, hooks_requested, mode, run.read_call_log(directory / "run-log.jsonl"))
    proof = result["isolation"]
    isolation_text = proof["status"]
    if isolation_text == "FAIL" and proof["hook_condition"] == "unknown":
        isolation_text += " (hooks unknown)"
    # Do not export tool arguments, answers, reference text, init payloads,
    # paths read by a measured model, or any other task/file content.
    return {"mode": mode, "request": request, "model": model,
            "received_form": result["received_form"], "verbatim": result["verbatim"],
            "extra_chars": result["extra_chars"], "isolation": isolation_text, "delivery": result["delivery"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", help="Saved run directories (read only)")
    args = parser.parse_args()
    for path in args.paths:
        try:
            row = score_saved(path)
        except (OSError, ValueError, KeyError):
            print(f"rescore_unavailable@{path}:0: missing or invalid scoring evidence", file=sys.stderr)
            return 2
        print(json.dumps(row, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
