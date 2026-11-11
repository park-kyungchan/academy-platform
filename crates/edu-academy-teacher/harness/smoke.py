"""Exactly two fake-CLI runs, no network/model sessions. Run separately from unit tests."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
FAKE = '''#!/usr/bin/env python3
import json, pathlib, sys
args = sys.argv[1:]
assert not list(pathlib.Path.cwd().iterdir()), "working directory must be empty"
assert "--no-session-persistence" in args and "--strict-mcp-config" in args
assert "--include-hook-events" in args
assert args[args.index("--output-format")+1] == "stream-json" and "--verbose" in args
config = json.loads(pathlib.Path(args[args.index("--mcp-config")+1]).read_text())
assert list(config["mcpServers"]) == ["academy"]
assert len(args[args.index("--allowedTools")+1].split(",")) == 14
request = sys.stdin.read()
if "--system-prompt" in args:
    assert "--bare" not in args
    assert args[args.index("--system-prompt")+1] == "You are an assistant of a teacher. Use the tools you are given."
    assert args[args.index("--setting-sources")+1] == ""
    assert args[args.index("--tools")+1] == "" and "--disable-slash-commands" in args
    allowed = args[args.index("--allowedTools")+1].split(",")
    print(json.dumps({"type":"system","subtype":"init","tools":allowed,"slash_commands":[],"skills":[],"plugins":[],"agents":["claude","Explore","general-purpose","Plan","statusline-setup"]}))
    print(json.dumps({"type":"assistant","message":{"usage":{"input_tokens":3,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"content":[{"type":"tool_use","id":"one","name":"mcp__academy__academy_lesson_prepare","input":{"student":"synthetic"}}]}}))
    print(json.dumps({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"one","content":[{"type":"text","text":"received stream display"}]}]}}))
    print(json.dumps({"type":"result","result":"prefix received stream display suffix 2026-11-09","usage":{"input_tokens":3,"output_tokens":4},"total_cost_usd":0.01}))
else:
    assert "--setting-sources" not in args and "--tools" not in args
    assert args[args.index("--disallowedTools")+1] == "Bash,Read,Grep,Glob,Edit,Write,NotebookEdit,Agent,WebFetch,WebSearch"
    print(json.dumps({"type":"system","subtype":"init"}))
'''


def main():
    with tempfile.TemporaryDirectory(prefix="p8-smoke-") as root:
        fake = Path(root) / "fake-cli"
        fake.write_text(FAKE)
        fake.chmod(0o700)
        for mode, request in (("isolated", "Q01"), ("installed", "Q02")):
            proc = subprocess.run([sys.executable, "-B", str(HERE / "run.py"), request,
                                   "fake", "--mode", mode, "--claude", str(fake)],
                                  capture_output=True, text=True, check=True, timeout=30)
            print(proc.stdout.strip())
            path = Path(proc.stdout.strip().split("run_dir=")[-1])
            result = json.loads((path / "measurement.json").read_text())
            assert result["exit_code"] == 0, (path / "claude.stderr").read_text()
            if mode == "isolated":
                assert result["call_count"] == 1 and result["args_bytes"] == 23
                assert result["verbatim"] == "pass"
                assert result["isolation"]["status"] == "pass"
                assert result["checks"]["previous_lesson_date"] == "pass"
                assert result["checks"]["line0"] == "FAIL"
            else:
                assert result["call_count"] == "unknown"
                assert result["verbatim"] == "unknown" and result["extra_chars"] == "unknown"
                assert set(result["checks"].values()) == {"unknown"}
    print("Two fake-CLI smoke runs passed")


if __name__ == "__main__":
    main()
