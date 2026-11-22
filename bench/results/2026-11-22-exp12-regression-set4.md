# Benchmark 2026-11-22

dogu 0.7.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit 8596840. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 4 (frozen before the change about printing whole results)

- dogu code mode, candidate build: 1% more input tokens, 1% lower cost and 2% more time than dogu serve (code mode).

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| dogu serve (code mode) | 21/21 | 6.8k | 14.4k | $0.0115 | 3.2s | 2 | 0 |
| dogu code mode, candidate build | 21/21 | 6.9k | 14.5k | $0.0113 | 3.3s | 2 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| dogu serve (code mode) | 0.25s | 3.20s | 0.83s | 2.2 |
| dogu code mode, candidate build | 0.28s | 3.16s | 0.77s | 2.2 |

Median total input tokens per task:

| Task | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|
| ruff-issues (a live count) | 21.4k (3/3 right) | 14.2k (3/3 right) |
| last-commit (a nested field) | 14.2k (3/3 right) | 14.5k (3/3 right) |
| pr-opener (one call) | 14.2k (3/3 right) | 14.5k (3/3 right) |
| most-forks (fan-out: 3 calls) | 14.4k (3/3 right) | 14.7k (3/3 right) |
| pushed-last (fan-out: 3 calls, compare) | 22.9k (3/3 right) | 23.3k (3/3 right) |
| ruff-section (finding one section) | 14.8k (3/3 right) | 15.1k (3/3 right) |
| entity-year (a lookup in a local server) | 13.6k (3/3 right) | 13.9k (3/3 right) |
