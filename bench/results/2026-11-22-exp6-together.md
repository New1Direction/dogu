# Benchmark 2026-11-22

dogu 0.6.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit 4cf7bb3. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 3 (frozen before the work on calls made together)

- dogu serve (code mode): 81% fewer input tokens, 61% lower cost and 51% more time than mcp servers loaded directly.
- dogu code mode, candidate build: 0% more input tokens, 0% higher cost and 19% less time than dogu serve (code mode).
- dogu code mode, candidate build: 81% fewer input tokens, 61% lower cost and 23% more time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 24/24 | 29.0k | 59.6k | $0.0296 | 3.5s | 2 | 0 |
| dogu serve (code mode) | 24/24 | 5.0k | 11.4k | $0.0115 | 5.3s | 2 | 0 |
| dogu code mode, candidate build | 24/24 | 5.2k | 11.5k | $0.0115 | 4.3s | 2 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.22s | 3.01s | 0.60s | 2.0 |
| dogu serve (code mode) | 0.26s | 3.35s | 1.48s | 2.4 |
| dogu code mode, candidate build | 0.25s | 3.33s | 0.99s | 2.4 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|---|
| most-stars (fan-out: 4 calls) | 59.7k (3/3 right) | 10.7k (3/3 right) | 11.5k (3/3 right) |
| three-tags (fan-out: 3 calls) | 63.3k (3/3 right) | 16.4k (3/3 right) | 16.9k (3/3 right) |
| big-repos (a count) | 61.8k (3/3 right) | 10.4k (3/3 right) | 10.7k (3/3 right) |
| first-pr (one call) | 58.5k (3/3 right) | 15.9k (3/3 right) | 16.3k (3/3 right) |
| bun-section (finding one section) | 59.6k (3/3 right) | 11.7k (3/3 right) | 12.0k (3/3 right) |
| started-in (a lookup in a local server) | 58.0k (3/3 right) | 10.6k (3/3 right) | 10.9k (3/3 right) |
| newer-sdk (fan-out: 2 calls, compare) | 63.2k (3/3 right) | 17.0k (3/3 right) | 17.5k (3/3 right) |
| most-issues (fan-out after a search) | 59.2k (3/3 right) | 11.1k (3/3 right) | 11.0k (3/3 right) |
