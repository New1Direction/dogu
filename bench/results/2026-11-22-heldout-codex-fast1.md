# Benchmark 2026-11-22

dogu 0.4.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 2 runs of each task per setup, bench at commit f19a60a. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- Codex, dogu folder in AGENTS.md: 24% fewer input tokens and 8% less time than codex, mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| Codex, MCP servers loaded directly | 16/16 | n/a | 68.4k | n/a | 10.0s | 2 | 7 |
| Codex, dogu folder in AGENTS.md | 16/16 | n/a | 52.3k | n/a | 9.2s | 2 | 1 |

Median total input tokens per task:

| Task | Codex, MCP servers loaded directly | Codex, dogu folder in AGENTS.md |
|---|---|---|
| created (one call, exact) | 49.7k (2/2 right) | 49.8k (2/2 right) |
| issue-title (one call, exact) | 61.8k (2/2 right) | 74.9k (2/2 right) |
| release-tags (long: 6+ calls) | 100.7k (2/2 right) | 78.2k (2/2 right) |
| docs-id (discovery, server not named) | 65.3k (2/2 right) | 50.4k (2/2 right) |
| deno-sections (one call, long answer) | 153.5k (2/2 right) | 50.6k (2/2 right) |
| memory-find (local server) | 62.9k (2/2 right) | 48.0k (2/2 right) |
| tokio-sections (two servers, a number) | 174.4k (2/2 right) | 75.5k (2/2 right) |
| axum-releases (big results, paging) | 70.9k (2/2 right) | 81.1k (2/2 right) |
