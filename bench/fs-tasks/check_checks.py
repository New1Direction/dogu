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


def todo_report(d):
    rows = sorted(
        (f.relative_to(d).as_posix(), i, line.strip())
        for f in (d / "src").rglob("*")
        if f.is_file()
        for i, line in enumerate(f.read_text(encoding="utf-8").splitlines(), 1)
        if "TODO" in line
    )
    put(d, "todo.txt", "".join(f"{p}:{i}: {l}\n" for p, i, l in rows))


def sales_total(d):
    import csv

    rows = [r for p in sorted((d / "data").glob("sales*.csv")) for r in csv.DictReader(p.open(encoding="utf-8"))]
    put(d, "total.txt", f"{sum(float(r['amount']) for r in rows):.2f}\n")


def doc_index(d):
    lines = []
    for p in sorted((d / "docs").glob("*.md")):
        heading = next(l for l in p.read_text(encoding="utf-8").splitlines() if l.startswith("#"))
        lines.append(f"- [{heading.lstrip('#').strip()}]({p.name})\n")
    put(d, "docs/INDEX.md", "".join(lines))


def sort_inbox(d):
    for p in [p for p in (d / "inbox").iterdir() if p.is_file()]:
        ext = p.name.rsplit(".", 1)[1] if "." in p.name else "none"
        (d / "inbox" / ext).mkdir(exist_ok=True)
        p.rename(d / "inbox" / ext / p.name)


def unique_emails(d):
    seen = []
    for line in text(d, "data/customers.txt").splitlines():
        if line.strip() and line.strip().lower() not in seen:
            seen.append(line.strip().lower())
    put(d, "unique.txt", "".join(s + "\n" for s in seen))


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
    "todo_report": todo_report,
    "error_count": lambda d: put(
        d,
        "errors.csv",
        "file,errors\n" + "".join(f"{p.name},{sum('ERROR' in l for l in p.read_text(encoding='utf-8').splitlines())}\n" for p in sorted((d / "logs").glob("*.log"))),
    ),
    "sales_total": sales_total,
    "doc_index": doc_index,
    "sort_inbox": sort_inbox,
    "find_setting": lambda d: put(d, "timeout.txt", "4500\n"),
    "unique_emails": unique_emails,
}


def passes(verify, d):
    env = {"FILESYSTEM_TEST_DIR": str(d), "PATH": os.environ.get("PATH", "/usr/bin:/bin")}
    return subprocess.run([sys.executable, "-I", str(verify)], env=env, capture_output=True).returncode == 0


def main():
    broken = []
    for verify in sorted(HERE.glob("tasks/filesystem/*/*/*/verify.py")):
        name = verify.parent.name
        state = json.loads((verify.parent / "meta.json").read_text())["meta_data"]["stateUrl"].removeprefix("local:")
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp) / state
            shutil.copytree(HERE / "states" / state, d)
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
