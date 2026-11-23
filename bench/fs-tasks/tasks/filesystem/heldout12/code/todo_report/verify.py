#!/usr/bin/env python3
"""Checker for dogu's held-out filesystem set 12 (MCPMark layout). Standard library only."""
import json, os, sys
from pathlib import Path

d = Path(os.environ["FILESYSTEM_TEST_DIR"])


def fail(why):
    print("FAIL: " + why)
    sys.exit(1)


def read(p):
    f = d / p
    if not f.is_file():
        fail(f"{p} not found")
    return f.read_text(encoding="utf-8")

rows = []
for f in sorted(p for p in (d / "src").rglob("*") if p.is_file()):
    for i, line in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
        if "TODO" in line:
            rows.append((f.relative_to(d).as_posix(), i, line.strip()))
rows.sort()
want = "".join(f"{p}:{i}: {l}\n" for p, i, l in rows)
if read("todo.txt") != want:
    fail("todo.txt is not every TODO line, exactly, in order")
print("PASS")
