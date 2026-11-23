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

want = ["file,errors"] + [f"{p.name},{sum('ERROR' in l for l in p.read_text(encoding='utf-8').splitlines())}" for p in sorted((d / "logs").glob("*.log"))]
got = [l.strip() for l in read("errors.csv").strip().splitlines()]
if got != want:
    fail(f"errors.csv is {got}, expected {want}")
print("PASS")
