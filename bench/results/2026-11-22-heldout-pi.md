# Benchmark 2026-11-22

dogu 0.3.0, Claude Code 2.1.330 (Claude Code), model `gpt-6.1-sol`, 2 runs of each task per setup, bench at commit 8295dde (the pi client is unchanged since f3268c5; the installed dogu was built at 5c081de). Model: openai/gpt-6.1-sol through pi, thinking off. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- pi, dogu folder in instructions: 50% fewer input tokens, 35% lower cost and 15% less time than pi, mcp tools declared directly.
- pi, dogu folder in instructions: 16% more input tokens, 24% higher cost and 21% less time than pi, its own code mode.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| pi, MCP tools declared directly | 16/16 | 11.4k | 24.1k | $0.0316 | 9.6s | 2 | 0 |
| pi, its own code mode | 16/16 | 1.1k | 10.4k | $0.0165 | 10.3s | 3 | 0 |
| pi, dogu folder in instructions | 16/16 | 5.6k | 12.0k | $0.0205 | 8.2s | 2 | 0 |

Median total input tokens per task:

| Task | pi, MCP tools declared directly | pi, its own code mode | pi, dogu folder in instructions |
|---|---|---|---|
| created (one call, exact) | 24.5k (2/2 right) | 10.2k (2/2 right) | 11.3k (2/2 right) |
| issue-title (one call, exact) | 23.1k (2/2 right) | 7.9k (2/2 right) | 14.3k (2/2 right) |
| release-tags (long: 6+ calls) | 46.6k (2/2 right) | 25.1k (2/2 right) | 25.7k (2/2 right) |
| docs-id (discovery, server not named) | 23.5k (2/2 right) | 10.4k (2/2 right) | 11.8k (2/2 right) |
| deno-sections (one call, long answer) | 23.7k (2/2 right) | 8.1k (2/2 right) | 12.0k (2/2 right) |
| memory-find (local server) | 23.0k (2/2 right) | 7.2k (2/2 right) | 11.2k (2/2 right) |
| tokio-sections (two servers, a number) | 35.2k (2/2 right) | 19.1k (2/2 right) | 17.5k (2/2 right) |
| axum-releases (big results, paging) | 36.1k (2/2 right) | 14.5k (2/2 right) | 17.1k (2/2 right) |
