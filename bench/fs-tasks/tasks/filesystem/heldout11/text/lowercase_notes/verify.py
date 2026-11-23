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

for p in sorted((d / "notes").glob("*.md")):
    if read(f"lower/{p.name}") != p.read_text(encoding="utf-8").lower():
        fail(f"lower/{p.name} is not notes/{p.name} in lowercase, exactly")
print("PASS")
