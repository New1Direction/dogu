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

hits = [p for p in (d / "archive").glob("*.txt") if "zephyrine" in p.read_text(encoding="utf-8")]
found = sorted(p.name for p in (d / "found").glob("*")) if (d / "found").is_dir() else []
if found != [hits[0].name]:
    fail(f"found/ holds {found}, expected [{hits[0].name!r}]")
if read(f"found/{hits[0].name}") != hits[0].read_text(encoding="utf-8"):
    fail("the copy is not the same as the original")
print("PASS")
