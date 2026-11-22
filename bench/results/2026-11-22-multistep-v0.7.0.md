# Benchmark 2026-11-22

dogu 0.7.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit 013ff13. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Multi-step tasks (frozen before any run with them)

- dogu folder, list in instructions: 58% fewer input tokens, 54% lower cost and 46% more time than mcp servers loaded directly.
- dogu serve (code mode): 72% fewer input tokens, 50% lower cost and 11% less time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 12/15 | 29.0k | 62.2k | $0.0347 | 4.3s | 4 | 0 |
| dogu folder, list in instructions | 15/15 | 12.7k | 26.1k | $0.0161 | 6.2s | 2 | 0 |
| dogu serve (code mode) | 12/15 | 6.8k | 17.6k | $0.0173 | 3.8s | 2 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.27s | 4.33s | 0.97s | 3.3 |
| dogu folder, list in instructions | 0.24s | 4.67s | 2.17s | 2.2 |
| dogu serve (code mode) | 0.26s | 3.95s | 1.80s | 2.6 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu folder, list in instructions | dogu serve (code mode) |
|---|---|---|---|
| deno-tags (a search, then 5 calls) | 92.7k (3/3 right) | 25.8k (3/3 right) | 31.9k (3/3 right) |
| astral-issues (adding up across repos) | 59.6k (3/3 right) | 26.0k (3/3 right) | 14.8k (3/3 right) |
| mcp-last-push (a search, then 5 calls, compare) | 91.3k (3/3 right) | 25.9k (3/3 right) | 23.2k (3/3 right) |
| most-sections (3 calls, compare) | 62.2k (0/3 right) | 29.3k (3/3 right) | 17.6k (0/3 right) |
| odd-years (filtering in a local server) | 58.5k (3/3 right) | 26.1k (3/3 right) | 14.1k (3/3 right) |
