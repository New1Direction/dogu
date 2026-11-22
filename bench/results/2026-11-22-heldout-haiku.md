# Benchmark 2026-11-22

dogu 0.3.0, Claude Code 2.1.330 (Claude Code), model `haiku`, 2 runs of each task per setup, bench at commit 7890a28 (the only uncommitted files under bench/ were the Sonnet results; later runs ignore bench/results/ in this check). Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- dogu folder, list in instructions: 55% fewer input tokens, 56% lower cost and 54% more time than mcp servers loaded directly.
- dogu folder, list in instructions: 5% more input tokens, 52% lower cost and 19% more time than loaded directly, claude code tool search on.
- dogu folder, "read the README": 20% fewer input tokens, 17% higher cost and 117% more time than mcp servers loaded directly.
- dogu folder, "read the README": 86% more input tokens, 29% higher cost and 67% more time than loaded directly, claude code tool search on.
- dogu serve (code mode): 70% fewer input tokens, 60% lower cost and 26% more time than mcp servers loaded directly.
- dogu serve (code mode): 31% fewer input tokens, 55% lower cost and 2% less time than loaded directly, claude code tool search on.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 16/16 | 29.8k | 60.8k | $0.002 | 3.6s | 2 | 0 |
| Loaded directly, Claude Code tool search on | 16/16 | 7.8k | 26.3k | $0.002 | 4.6s | 3 | 0 |
| dogu folder, list in instructions | 16/16 | 13.0k | 27.6k | $0.001 | 5.5s | 2 | 0 |
| dogu folder, "read the README" | 16/16 | 6.7k | 48.9k | $0.003 | 7.7s | 4 | 0 |
| dogu serve (code mode) | 16/16 | 5.1k | 18.2k | $0.001 | 4.5s | 3 | 0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | Loaded directly, Claude Code tool search on | dogu folder, list in instructions | dogu folder, "read the README" | dogu serve (code mode) |
|---|---|---|---|---|---|
| created (one call, exact) | 60.2k (2/2 right) | 25.6k (2/2 right) | 34.0k (2/2 right) | 41.8k (2/2 right) | 17.7k (2/2 right) |
| issue-title (one call, exact) | 60.1k (2/2 right) | 25.7k (2/2 right) | 40.3k (2/2 right) | 59.4k (2/2 right) | 20.2k (2/2 right) |
| release-tags (long: 6+ calls) | 106.9k (2/2 right) | 52.4k (2/2 right) | 41.1k (2/2 right) | 48.9k (2/2 right) | 19.0k (2/2 right) |
| docs-id (discovery, server not named) | 60.6k (2/2 right) | 27.0k (2/2 right) | 27.2k (2/2 right) | 49.3k (2/2 right) | 19.5k (2/2 right) |
| deno-sections (one call, long answer) | 61.1k (2/2 right) | 25.7k (2/2 right) | 27.6k (2/2 right) | 35.4k (2/2 right) | 18.2k (2/2 right) |
| memory-find (local server) | 59.8k (2/2 right) | 24.7k (2/2 right) | 26.4k (2/2 right) | 34.1k (2/2 right) | 16.5k (2/2 right) |
| tokio-sections (two servers, a number) | 93.7k (2/2 right) | 38.2k (2/2 right) | 41.3k (2/2 right) | 52.2k (2/2 right) | 21.7k (2/2 right) |
| axum-releases (big results, paging) | 98.3k (2/2 right) | 36.4k (2/2 right) | 26.5k (2/2 right) | 64.9k (2/2 right) | 16.8k (2/2 right) |
