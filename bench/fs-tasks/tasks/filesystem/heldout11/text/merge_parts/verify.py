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

want = read("parts/part1.txt") + read("parts/part2.txt") + read("parts/part3.txt")
if read("whole.txt") != want:
    fail("whole.txt is not the three parts joined exactly")
print("PASS")
