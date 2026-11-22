# Benchmark 2026-11-22

dogu 0.7.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit dd41b25. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 8 (frozen before the Promise.allSettled change)

- dogu serve (code mode): 74% fewer input tokens, 62% lower cost and 19% more time than mcp servers loaded directly.
- dogu code mode, candidate build: 39% more input tokens, 24% higher cost and 14% more time than dogu serve (code mode).
- dogu code mode, candidate build: 64% fewer input tokens, 53% lower cost and 36% more time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 15/15 | 27.2k | 86.0k | $0.0451 | 7.3s | 7 | 0 |
| dogu serve (code mode) | 15/15 | 6.6k | 22.6k | $0.0172 | 8.7s | 3 | 0 |
| dogu code mode, candidate build | 15/15 | 6.8k | 31.3k | $0.0213 | 10.0s | 4 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.22s | 5.99s | 1.48s | 4.7 |
| dogu serve (code mode) | 0.24s | 5.38s | 3.17s | 3.0 |
| dogu code mode, candidate build | 0.24s | 6.72s | 3.94s | 3.6 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|---|
| oven-tags (fan-out where some calls fail) | 86.0k (3/3 right) | 22.8k (3/3 right) | 32.3k (3/3 right) |
| encode-tags (fan-out where some calls fail) | 85.8k (3/3 right) | 22.8k (3/3 right) | 31.5k (3/3 right) |
| vite-tags (fan-out where some calls fail) | 88.1k (3/3 right) | 22.6k (3/3 right) | 23.1k (3/3 right) |
| encode-newest (fan-out where some calls fail, compare) | 86.8k (3/3 right) | 23.4k (3/3 right) | 33.3k (3/3 right) |
| mixed-tags (5 calls, one fails) | 62.2k (3/3 right) | 21.9k (3/3 right) | 23.0k (3/3 right) |
