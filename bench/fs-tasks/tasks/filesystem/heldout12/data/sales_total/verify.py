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

import csv
total = sum(float(r["amount"]) for p in sorted((d / "data").glob("sales*.csv")) for r in csv.DictReader(p.open(encoding="utf-8")))
if read("total.txt").strip() != f"{total:.2f}":
    fail(f"total.txt is {read('total.txt').strip()!r}, expected {total:.2f}")
print("PASS")
