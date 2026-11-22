# Benchmark 2026-11-22

dogu 0.7.1, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit 48b2075. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 4 (frozen before the change about printing whole results)

- dogu serve (code mode): 75% fewer input tokens, 54% lower cost and 6% more time than mcp servers loaded directly.
- dogu code mode, candidate build: 0% more input tokens, 1% higher cost and 5% less time than dogu serve (code mode).
- dogu code mode, candidate build: 75% fewer input tokens, 53% lower cost and 0% more time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 21/21 | 29.0k | 58.6k | $0.0245 | 3.5s | 2 | 0 |
| dogu serve (code mode) | 21/21 | 6.9k | 14.7k | $0.0113 | 3.7s | 2 | 0 |
| dogu code mode, candidate build | 21/21 | 6.9k | 14.7k | $0.0114 | 3.5s | 2 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.34s | 3.14s | 0.57s | 2.2 |
| dogu serve (code mode) | 0.30s | 3.55s | 1.00s | 2.3 |
| dogu code mode, candidate build | 0.32s | 3.36s | 0.61s | 2.2 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|---|
| ruff-issues (a live count) | 58.6k (3/3 right) | 21.5k (3/3 right) | 21.9k (3/3 right) |
| last-commit (a nested field) | 58.5k (3/3 right) | 14.5k (3/3 right) | 14.6k (3/3 right) |
| pr-opener (one call) | 58.6k (3/3 right) | 14.5k (3/3 right) | 14.0k (3/3 right) |
| most-forks (fan-out: 3 calls) | 59.3k (3/3 right) | 14.7k (3/3 right) | 14.7k (3/3 right) |
| pushed-last (fan-out: 3 calls, compare) | 60.3k (3/3 right) | 23.1k (3/3 right) | 23.3k (3/3 right) |
| ruff-section (finding one section) | 59.2k (3/3 right) | 15.1k (3/3 right) | 15.1k (3/3 right) |
| entity-year (a lookup in a local server) | 58.0k (3/3 right) | 13.9k (3/3 right) | 13.9k (3/3 right) |
