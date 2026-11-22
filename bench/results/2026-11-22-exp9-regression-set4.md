# Benchmark 2026-11-22

dogu 0.6.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit b5c457a. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 4 (frozen before the change about printing whole results)

- dogu serve (code mode): 81% fewer input tokens, 56% lower cost and 10% less time than mcp servers loaded directly.
- dogu code mode, candidate build: 26% more input tokens, 5% higher cost and 2% less time than dogu serve (code mode).
- dogu code mode, candidate build: 75% fewer input tokens, 54% lower cost and 12% less time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 21/21 | 29.0k | 58.6k | $0.0250 | 3.9s | 2 | 0 |
| dogu serve (code mode) | 19/21 | 5.3k | 11.4k | $0.0110 | 3.5s | 2 | 0 |
| dogu code mode, candidate build | 21/21 | 6.8k | 14.4k | $0.0116 | 3.4s | 2 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.30s | 3.51s | 0.64s | 2.3 |
| dogu serve (code mode) | 0.30s | 3.27s | 0.79s | 2.2 |
| dogu code mode, candidate build | 0.29s | 3.25s | 0.93s | 2.3 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|---|
| ruff-issues (a live count) | 58.6k (3/3 right) | 16.3k (1/3 right) | 21.4k (3/3 right) |
| last-commit (a nested field) | 58.5k (3/3 right) | 11.2k (3/3 right) | 14.2k (3/3 right) |
| pr-opener (one call) | 58.6k (3/3 right) | 11.2k (3/3 right) | 14.2k (3/3 right) |
| most-forks (fan-out: 3 calls) | 89.1k (3/3 right) | 11.4k (3/3 right) | 14.4k (3/3 right) |
| pushed-last (fan-out: 3 calls, compare) | 60.3k (3/3 right) | 18.7k (3/3 right) | 22.8k (3/3 right) |
| ruff-section (finding one section) | 59.2k (3/3 right) | 11.8k (3/3 right) | 14.8k (3/3 right) |
| entity-year (a lookup in a local server) | 58.0k (3/3 right) | 10.6k (3/3 right) | 13.6k (3/3 right) |
