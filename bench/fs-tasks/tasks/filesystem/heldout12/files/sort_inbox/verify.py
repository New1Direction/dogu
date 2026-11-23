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

want = {"txt/report.pdf.txt": "not really a pdf\n", "jpg/photo.jpg": "JPEGDATA\n", "txt/notes.txt": "remember the milk\n", "none/Makefile": "all:\n\techo hi\n", "csv/data.csv": "a,b\n1,2\n", "gz/backup.tar.gz": "TARDATA\n"}
got = {p.relative_to(d / "inbox").as_posix(): p.read_text(encoding="utf-8") for p in (d / "inbox").rglob("*") if p.is_file()}
if got != want:
    fail(f"inbox/ holds {sorted(got)}, expected {sorted(want)} with unchanged content")
print("PASS")
