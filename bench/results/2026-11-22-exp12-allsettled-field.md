# Benchmark 2026-11-22

dogu 0.7.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit 725ad79. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 9 (frozen before experiment 12)

- dogu serve (code mode): 74% fewer input tokens, 70% lower cost and 12% more time than mcp servers loaded directly.
- dogu code mode, candidate build: 2% more input tokens, 11% lower cost and 5% less time than dogu serve (code mode).
- dogu code mode, candidate build: 74% fewer input tokens, 73% lower cost and 7% more time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 15/15 | 27.2k | 86.5k | $0.0575 | 7.4s | 7 | 0 |
| dogu serve (code mode) | 15/15 | 6.6k | 22.4k | $0.0173 | 8.4s | 3 | 0 |
| dogu code mode, candidate build | 15/15 | 6.8k | 22.8k | $0.0154 | 8.0s | 3 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.30s | 5.79s | 1.57s | 4.8 |
| dogu serve (code mode) | 0.28s | 5.31s | 3.99s | 3.2 |
| dogu code mode, candidate build | 0.27s | 4.87s | 2.90s | 2.7 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|---|
| svelte-tags (fan-out where some calls fail) | 85.4k (3/3 right) | 21.7k (3/3 right) | 22.8k (3/3 right) |
| tauri-tags (fan-out where some calls fail) | 106.2k (3/3 right) | 22.6k (3/3 right) | 23.0k (3/3 right) |
| nushell-tags (fan-out where some calls fail) | 88.3k (3/3 right) | 22.5k (3/3 right) | 14.2k (3/3 right) |
| hono-tags (fan-out where some calls fail) | 86.5k (3/3 right) | 22.4k (3/3 right) | 14.0k (3/3 right) |
| astro-newest (fan-out where some calls fail, compare) | 85.7k (3/3 right) | 22.4k (3/3 right) | 22.8k (3/3 right) |
