# Benchmark 2026-11-22

dogu 0.4.0, Claude Code 2.1.330 (Claude Code), model `gpt-6.1-terra`, 2 runs of each task per setup, bench at commit 2490b98. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- Codex, dogu folder in AGENTS.md: 47% fewer input tokens and 13% less time than codex, mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| Codex, MCP servers loaded directly | 14/16 | n/a | 92.4k | n/a | 8.3s | 2 | 3 |
| Codex, dogu folder in AGENTS.md | 16/16 | n/a | 48.9k | n/a | 7.3s | 2 | 0 |

Median total input tokens per task:

| Task | Codex, MCP servers loaded directly | Codex, dogu folder in AGENTS.md |
|---|---|---|
| created (one call, exact) | 107.9k (2/2 right) | 46.9k (2/2 right) |
| issue-title (one call, exact) | 93.2k (2/2 right) | 70.5k (2/2 right) |
| release-tags (long: 6+ calls) | 162.9k (2/2 right) | 68.4k (2/2 right) |
| docs-id (discovery, server not named) | 61.5k (2/2 right) | 47.4k (2/2 right) |
| deno-sections (one call, long answer) | 64.1k (0/2 right) | 47.6k (2/2 right) |
| memory-find (local server) | 77.1k (2/2 right) | 46.8k (2/2 right) |
| tokio-sections (two servers, a number) | 268.5k (2/2 right) | 68.6k (2/2 right) |
| axum-releases (big results, paging) | 84.5k (2/2 right) | 45.2k (2/2 right) |
