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

lines = []
for p in sorted((d / "docs").glob("*.md")):
    if p.name == "INDEX.md":
        continue
    h = next(l for l in p.read_text(encoding="utf-8").splitlines() if l.startswith("#"))
    lines.append(f"- [{h.lstrip('#').strip()}]({p.name})\n")
if read("docs/INDEX.md") != "".join(lines):
    fail("docs/INDEX.md is not the expected index, exactly")
print("PASS")
