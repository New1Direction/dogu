# Benchmark 2026-11-22

dogu 0.6.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit c3dc617. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 3 (frozen before the work on calls made together)

- dogu folder, list in instructions: 57% fewer input tokens, 61% lower cost and 67% more time than mcp servers loaded directly.
- dogu folder, candidate build: 0% more input tokens, 1% lower cost and 22% less time than dogu folder, list in instructions.
- dogu folder, candidate build: 57% fewer input tokens, 61% lower cost and 30% more time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 24/24 | 29.0k | 59.6k | $0.0295 | 3.4s | 2 | 0 |
| dogu folder, list in instructions | 24/24 | 12.7k | 25.8k | $0.0115 | 5.6s | 2 | 0 |
| dogu folder, candidate build | 24/24 | 12.7k | 25.8k | $0.0114 | 4.4s | 2 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.20s | 2.86s | 0.60s | 2.1 |
| dogu folder, list in instructions | 0.23s | 4.04s | 1.69s | 2.0 |
| dogu folder, candidate build | 0.23s | 3.43s | 1.19s | 2.0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu folder, list in instructions | dogu folder, candidate build |
|---|---|---|---|
| most-stars (fan-out: 4 calls) | 59.7k (3/3 right) | 25.8k (3/3 right) | 25.8k (3/3 right) |
| three-tags (fan-out: 3 calls) | 63.3k (3/3 right) | 25.7k (3/3 right) | 25.7k (3/3 right) |
| big-repos (a count) | 61.8k (3/3 right) | 25.8k (3/3 right) | 26.0k (3/3 right) |
| first-pr (one call) | 58.5k (3/3 right) | 25.6k (3/3 right) | 25.6k (3/3 right) |
| bun-section (finding one section) | 59.6k (3/3 right) | 27.1k (3/3 right) | 27.1k (3/3 right) |
| started-in (a lookup in a local server) | 58.0k (3/3 right) | 25.6k (3/3 right) | 25.6k (3/3 right) |
| newer-sdk (fan-out: 2 calls, compare) | 63.2k (3/3 right) | 25.7k (3/3 right) | 25.8k (3/3 right) |
| most-issues (fan-out after a search) | 59.2k (3/3 right) | 25.8k (3/3 right) | 25.8k (3/3 right) |
