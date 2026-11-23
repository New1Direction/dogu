#!/usr/bin/env python3
"""Checker for dogu's held-out filesystem set 11 (MCPMark layout). Standard library only."""
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

rows = read("counts.csv").strip().splitlines()
want = ["file,words"] + [f"{p.name},{len(p.read_text(encoding='utf-8').split())}" for p in sorted((d / "docs").glob("*.txt"))]
if [r.strip() for r in rows] != want:
    fail(f"counts.csv is {rows}, expected {want}")
print("PASS")
