# Benchmark 2026-11-22

dogu 0.4.0, Claude Code 2.1.330 (Claude Code), model `gpt-6.1-terra`, 2 runs of each task per setup, bench at commit 2490b98. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- Codex, dogu folder in AGENTS.md: 53% fewer input tokens and 19% less time than codex, mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| Codex, MCP servers loaded directly | 15/16 | n/a | 104.4k | n/a | 13.7s | 2 | 6 |
| Codex, dogu folder in AGENTS.md | 16/16 | n/a | 49.4k | n/a | 11.1s | 2 | 0 |

Median total input tokens per task:

| Task | Codex, MCP servers loaded directly | Codex, dogu folder in AGENTS.md |
|---|---|---|
| created (one call, exact) | 108.1k (2/2 right) | 45.1k (2/2 right) |
| issue-title (one call, exact) | 72.3k (2/2 right) | 70.5k (2/2 right) |
| release-tags (long: 6+ calls) | 157.1k (2/2 right) | 71.1k (2/2 right) |
| docs-id (discovery, server not named) | 61.5k (2/2 right) | 47.4k (2/2 right) |
| deno-sections (one call, long answer) | 77.6k (1/2 right) | 49.4k (2/2 right) |
| memory-find (local server) | 77.1k (2/2 right) | 48.6k (2/2 right) |
| tokio-sections (two servers, a number) | 218.6k (2/2 right) | 71.2k (2/2 right) |
| axum-releases (big results, paging) | 128.5k (2/2 right) | 58.3k (2/2 right) |
