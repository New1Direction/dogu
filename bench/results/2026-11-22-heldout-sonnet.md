# Benchmark 2026-11-22

dogu 0.3.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit 7890a28. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- dogu folder, list in instructions: 57% fewer input tokens, 62% lower cost and 23% more time than mcp servers loaded directly.
- dogu folder, list in instructions: 6% more input tokens, 65% lower cost and 8% less time than loaded directly, claude code tool search on.
- dogu folder, "read the README": 46% fewer input tokens, 23% higher cost and 69% more time than mcp servers loaded directly.
- dogu folder, "read the README": 36% more input tokens, 15% higher cost and 26% more time than loaded directly, claude code tool search on.
- dogu serve (code mode): 75% fewer input tokens, 64% lower cost and 33% more time than mcp servers loaded directly.
- dogu serve (code mode): 36% fewer input tokens, 66% lower cost and 1% less time than loaded directly, claude code tool search on.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 24/24 | 29.0k | 59.1k | $0.034 | 4.2s | 2 | 0 |
| Loaded directly, Claude Code tool search on | 24/24 | 7.0k | 23.6k | $0.037 | 5.6s | 3 | 0 |
| dogu folder, list in instructions | 24/24 | 12.1k | 25.1k | $0.013 | 5.1s | 2 | 0 |
| dogu folder, "read the README" | 24/24 | 5.9k | 32.0k | $0.042 | 7.0s | 3 | 0 |
| dogu serve (code mode) | 24/24 | 4.3k | 15.0k | $0.013 | 5.5s | 3 | 0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | Loaded directly, Claude Code tool search on | dogu folder, list in instructions | dogu folder, "read the README" | dogu serve (code mode) |
|---|---|---|---|---|---|
| created (one call, exact) | 58.4k (3/3 right) | 22.7k (3/3 right) | 24.6k (3/3 right) | 31.5k (3/3 right) | 14.6k (3/3 right) |
| issue-title (one call, exact) | 58.3k (3/3 right) | 22.8k (3/3 right) | 37.6k (3/3 right) | 44.9k (3/3 right) | 20.2k (3/3 right) |
| release-tags (long: 6+ calls) | 103.0k (3/3 right) | 47.6k (3/3 right) | 24.9k (3/3 right) | 31.7k (3/3 right) | 15.4k (3/3 right) |
| docs-id (discovery, server not named) | 58.8k (3/3 right) | 24.2k (3/3 right) | 25.4k (3/3 right) | 32.2k (3/3 right) | 16.0k (3/3 right) |
| deno-sections (one call, long answer) | 59.3k (3/3 right) | 23.0k (3/3 right) | 25.8k (3/3 right) | 46.9k (3/3 right) | 14.7k (3/3 right) |
| memory-find (local server) | 58.0k (3/3 right) | 21.7k (3/3 right) | 24.6k (3/3 right) | 31.5k (3/3 right) | 8.8k (3/3 right) |
| tokio-sections (two servers, a number) | 89.3k (3/3 right) | 32.7k (3/3 right) | 38.3k (3/3 right) | 45.6k (3/3 right) | 15.6k (3/3 right) |
| axum-releases (big results, paging) | 62.2k (3/3 right) | 36.0k (3/3 right) | 24.6k (3/3 right) | 31.5k (3/3 right) | 13.9k (3/3 right) |
