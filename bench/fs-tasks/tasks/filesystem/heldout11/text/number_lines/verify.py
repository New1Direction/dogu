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

lines = read("todo.txt").splitlines()
want = "".join(f"{i}: {l}\n" for i, l in enumerate(lines, 1))
if read("numbered.txt") != want:
    fail("numbered.txt is not todo.txt's lines numbered, exactly")
print("PASS")
