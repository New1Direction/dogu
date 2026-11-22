# Benchmark 2026-11-22

dogu 0.7.0, Claude Code 2.1.330 (Claude Code), model `haiku`, 3 runs of each task per setup, bench at commit 013ff13. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- dogu folder, list in instructions: 53% fewer input tokens, 53% lower cost and 57% more time than mcp servers loaded directly.
- dogu folder, list in instructions: 12% more input tokens, 50% lower cost and 14% more time than loaded directly, claude code tool search on.
- dogu serve (code mode): 73% fewer input tokens, 57% lower cost and 21% more time than mcp servers loaded directly.
- dogu serve (code mode): 35% fewer input tokens, 54% lower cost and 12% less time than loaded directly, claude code tool search on.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 21/21 | 29.8k | 61.0k | $0.0024 | 3.0s | 2 | 0 |
| Loaded directly, Claude Code tool search on | 21/21 | 7.8k | 25.7k | $0.0023 | 4.1s | 3 | 0 |
| dogu folder, list in instructions | 21/21 | 13.5k | 28.9k | $0.0011 | 4.7s | 3 | 0 |
| dogu serve (code mode) | 21/21 | 7.6k | 16.6k | $0.0010 | 3.6s | 2 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.23s | 3.79s | 0.70s | 2.5 |
| Loaded directly, Claude Code tool search on | 1.23s | 4.51s | 0.79s | 3.6 |
| dogu folder, list in instructions | 0.23s | 3.54s | 1.51s | 2.5 |
| dogu serve (code mode) | 0.25s | 3.19s | 0.97s | 2.4 |

Median total input tokens per task:

| Task | MCP servers loaded directly | Loaded directly, Claude Code tool search on | dogu folder, list in instructions | dogu serve (code mode) |
|---|---|---|---|---|
| created (one call, exact) | 60.2k (3/3 right) | 25.6k (3/3 right) | 27.4k (3/3 right) | 15.7k (3/3 right) |
| issue-title (one call, exact) | 60.1k (3/3 right) | 25.7k (3/3 right) | 41.9k (3/3 right) | 15.7k (3/3 right) |
| release-tags (long: 6+ calls) | 105.8k (3/3 right) | 52.0k (3/3 right) | 42.8k (3/3 right) | 27.1k (3/3 right) |
| deno-sections (one call, long answer) | 61.0k (3/3 right) | 25.7k (3/3 right) | 28.5k (3/3 right) | 16.6k (3/3 right) |
| memory-find (local server) | 59.7k (3/3 right) | 24.6k (3/3 right) | 27.3k (3/3 right) | 15.6k (3/3 right) |
| tokio-sections (two servers, a number) | 92.9k (3/3 right) | 37.3k (3/3 right) | 42.8k (3/3 right) | 26.9k (3/3 right) |
| axum-releases (big results, paging) | 95.9k (3/3 right) | 43.5k (3/3 right) | 27.8k (3/3 right) | 23.9k (3/3 right) |
