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

seen = []
for l in read("data/customers.txt").splitlines():
    if l.strip() and l.strip().lower() not in seen:
        seen.append(l.strip().lower())
if read("unique.txt") != "".join(s + "\n" for s in seen):
    fail(f"unique.txt is {read('unique.txt')!r}")
print("PASS")
