# Benchmark 2026-11-22

dogu 0.5.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit 929ee82. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 2 (frozen before the code-mode speed work)

- dogu serve (code mode): 74% fewer input tokens, 59% lower cost and 28% more time than mcp servers loaded directly.
- dogu code mode, candidate build: 30% fewer input tokens, 10% lower cost and 20% less time than dogu serve (code mode).
- dogu code mode, candidate build: 82% fewer input tokens, 63% lower cost and 2% more time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 21/21 | 29.0k | 59.2k | $0.0303 | 3.3s | 2 | 0 |
| dogu serve (code mode) | 21/21 | 4.6k | 15.6k | $0.0124 | 4.3s | 3 | 0 |
| dogu code mode, candidate build | 21/21 | 5.0k | 10.9k | $0.0111 | 3.4s | 2 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.21s | 3.13s | 0.58s | 2.2 |
| dogu serve (code mode) | 0.25s | 3.54s | 1.18s | 2.7 |
| dogu code mode, candidate build | 0.26s | 3.59s | 1.44s | 2.3 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|---|
| open-prs (a count) | 58.1k (3/3 right) | 17.7k (3/3 right) | 15.8k (3/3 right) |
| oldest-issue (sorting) | 58.4k (3/3 right) | 15.9k (3/3 right) | 10.5k (3/3 right) |
| release-author (a nested field) | 59.2k (3/3 right) | 14.7k (3/3 right) | 10.9k (3/3 right) |
| stars-sum (adding up) | 59.9k (3/3 right) | 15.6k (3/3 right) | 15.8k (3/3 right) |
| deepwiki-node (finding one section) | 59.4k (3/3 right) | 10.7k (3/3 right) | 11.5k (3/3 right) |
| memory-years (counting in a local server) | 58.5k (3/3 right) | 9.9k (3/3 right) | 10.6k (3/3 right) |
| newest-release (comparing across calls) | 99.8k (3/3 right) | 16.3k (3/3 right) | 16.9k (3/3 right) |
