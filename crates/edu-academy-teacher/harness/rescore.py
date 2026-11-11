#!/usr/bin/env python3
"""Offline scoring: only row values leave saved evidence directories."""
from collections import Counter
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile

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
            "received_form": result["received_form"], "shown_unchanged": result["shown_unchanged"], "facts_kept": result["facts_kept"],
            "source_footer": result["source_footer"], "closing_statement": result["closing_statement"],
            "reason_code": result["reason_code"], "unknown_by_reason": result["unknown_by_reason"], "unit_counts": result["unit_counts"],
            "failed_request_checks": run.failed_request_checks(result),
            "failed_rules": result.get("failed_rules", []), "extra_chars": result["extra_chars"], "isolation": isolation_text, "delivery": result["delivery"]}


def rebuild_rows(path):
    """Replay saved turns, adding one field without rewriting existing bytes."""
    import diffclass

    original = path.read_bytes()
    lines = original.splitlines(keepends=True)
    if len(lines) > diffclass.MAX_ROWS:
        raise ValueError("row_limit")
    tools = json.loads((run.CRATE / "tests/fixtures/tools-list.json").read_text())["tools"]
    allowed = ["mcp__academy__" + tool["name"] for tool in tools]
    specs = json.loads((run.HERE / "checks.json").read_text())
    rewritten, cache = [], {}
    for line in lines:
        row = json.loads(line)
        if "failed_request_checks" in row:
            raise ValueError("failed_checks_already_present")
        directory = diffclass.resolve(row)
        if directory not in cache:
            saved = json.loads(diffclass.read_bounded(directory / "measurement.json", diffclass.MAX_METADATA_BYTES))
            raw = diffclass.read_bounded(directory / "claude.jsonl", diffclass.MAX_STREAM_BYTES)
            log = [json.loads(item) for item in diffclass.read_bounded(directory / "run-log.jsonl", diffclass.MAX_LOG_BYTES).splitlines()]
            cache[directory] = run.score_turns(raw, specs[row["request"]]["turns"], allowed,
                                              row["mode"], log, saved.get("error"), saved.get("sent_turns"))
        result = cache[directory][row["turn"] - 1]
        names = run.failed_request_checks(result)
        replayed = dict(run.public_row(result, row["request"],
                        {"low": "haiku", "reference": "sonnet"}[row["model_tier"]],
                        row["mode"], row["conversation"]), trial=row["trial"])
        if any(replayed.get(key) != value for key, value in row.items()):
            raise ValueError("prior_row_changed")
        end = line.rfind(b"}")
        if end < 0:
            raise ValueError("invalid_row_object")
        addition = b', "failed_request_checks": ' + json.dumps(names, ensure_ascii=False).encode("utf-8")
        changed = line[:end] + addition + line[end:]
        # Exact byte preservation, including key order, escapes, spacing and EOL.
        assert changed[:end] + changed[end + len(addition):] == line
        recovered = json.loads(changed)
        assert recovered.pop("failed_request_checks") == names and recovered == row
        rewritten.append(changed)
    backup = path.with_name(path.stem + ".before-failed-checks.jsonl")
    fd = os.open(backup, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "wb") as output:
        output.write(original)
    if backup.read_bytes() != original or path.read_bytes() != original:
        raise ValueError("preimage_changed")
    fd, name = tempfile.mkstemp(prefix=path.name + ".", dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(fd, "wb") as output:
            output.write(b"".join(rewritten))
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)
    print(json.dumps({"rows_rebuilt": len(lines), "other_field_bytes_unchanged": True}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", help="Saved run directories (read only)")
    parser.add_argument("--rebuild-rows", type=Path, help="Add failed check names from saved evidence; preserve prior bytes")
    args = parser.parse_args()
    if args.rebuild_rows:
        if args.paths:
            parser.error("--rebuild-rows does not accept positional paths")
        try:
            rebuild_rows(args.rebuild_rows)
        except (OSError, ValueError, KeyError, TypeError, AssertionError):
            print("row_rebuild_unavailable@rescore.py:0: saved evidence or prior row mismatch", file=sys.stderr)
            return 2
        return 0
    if not args.paths:
        parser.error("saved paths or --rebuild-rows required")
    unknown_by_reason=Counter()
    for path in args.paths:
        try:
            row = score_saved(path)
        except (OSError, ValueError, KeyError):
            print(f"rescore_unavailable@{path}:0: missing or invalid scoring evidence", file=sys.stderr)
            return 2
        unknown_by_reason.update(row["unknown_by_reason"])
        print(json.dumps(row, ensure_ascii=False))
    print(json.dumps({"unknown_by_reason":dict(sorted(unknown_by_reason.items()))}),file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
