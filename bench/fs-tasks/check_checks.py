#!/usr/bin/env python3
"""Checks the checkers of bench/fs-tasks: each must reject the untouched starting files and accept a
right solution, made here by plain Python. A checker that fails either way would make every result
measured with it wrong.

    python3 bench/fs-tasks/check_checks.py
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).parent
STATE = HERE / "states" / "workspace"


def text(d, p):
    return (d / p).read_text(encoding="utf-8")


def put(d, p, s):
    (d / p).parent.mkdir(parents=True, exist_ok=True)
    (d / p).write_text(s, encoding="utf-8")


def bump(d):
    cfg = json.loads(text(d, "config.json"))
    put(d, "config.json", json.dumps({**cfg, "version": "2.0.0"}, indent=2) + "\n")


def find(d):
    hit = next(p for p in sorted((d / "archive").glob("*.txt")) if "zephyrine" in p.read_text(encoding="utf-8"))
    put(d, f"found/{hit.name}", hit.read_text(encoding="utf-8"))


SOLUTIONS = {
    "reverse_lines": lambda d: put(d, "reversed.txt", "\n".join(reversed(text(d, "poem.txt").splitlines())) + "\n"),
    "merge_parts": lambda d: put(d, "whole.txt", "".join(text(d, f"parts/part{i}.txt") for i in (1, 2, 3))),
    "word_counts": lambda d: put(
        d,
        "counts.csv",
        "file,words\n" + "".join(f"{p.name},{len(p.read_text(encoding='utf-8').split())}\n" for p in sorted((d / "docs").glob("*.txt"))),
    ),
    "lowercase_notes": lambda d: [put(d, f"lower/{p.name}", p.read_text(encoding="utf-8").lower()) for p in sorted((d / "notes").glob("*.md"))],
    "find_word": find,
    "number_lines": lambda d: put(d, "numbered.txt", "".join(f"{i}: {l}\n" for i, l in enumerate(text(d, "todo.txt").splitlines(), 1))),
    "bump_version": bump,
}


def passes(verify, d):
    env = {"FILESYSTEM_TEST_DIR": str(d), "PATH": os.environ.get("PATH", "/usr/bin:/bin")}
    return subprocess.run([sys.executable, "-I", str(verify)], env=env, capture_output=True).returncode == 0


def main():
    broken = []
    for verify in sorted(HERE.glob("tasks/filesystem/*/*/*/verify.py")):
        name = verify.parent.name
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp) / "workspace"
            shutil.copytree(STATE, d)
            if passes(verify, d):
                broken.append(f"{name}: accepts the untouched starting files")
            SOLUTIONS[name](d)
            if not passes(verify, d):
                broken.append(f"{name}: rejects the right solution")
    for b in broken:
        print("BROKEN " + b)
    print(f"{len(SOLUTIONS) - len({b.split(':')[0] for b in broken})} of {len(SOLUTIONS)} checkers are sound")
    sys.exit(1 if broken else 0)


main()
