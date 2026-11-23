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

got = json.loads(read("config.json"))
if got.get("version") != "2.0.0":
    fail(f"version is {got.get('version')!r}")
want = {"name": "orchard-sync", "version": "2.0.0", "features": {"retry": {"max": 5, "backoffMs": 250}, "telemetry": False, "regions": ["eu-west", "ap-south"]}, "owners": ["ines", "tomas"], "limits": {"daily": 10000, "burst": 120.5}, "notes": "keep in sync with deploy/values.yaml"}
if got != want:
    fail("other keys or values changed")
print("PASS")
