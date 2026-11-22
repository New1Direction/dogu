# Benchmark 2026-11-22

dogu 0.4.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 2 runs of each task per setup, bench at commit 1bcc43c with uncommitted changes. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- Codex, dogu folder in AGENTS.md: 30% fewer input tokens and 18% less time than codex, mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| Codex, MCP servers loaded directly | 16/16 | n/a | 74.1k | n/a | 13.8s | 2 | 4 |
| Codex, dogu folder in AGENTS.md | 16/16 | n/a | 51.9k | n/a | 11.2s | 2 | 0 |

Median total input tokens per task:

| Task | Codex, MCP servers loaded directly | Codex, dogu folder in AGENTS.md |
|---|---|---|
| created (one call, exact) | 71.2k (2/2 right) | 48.0k (2/2 right) |
| issue-title (one call, exact) | 61.7k (2/2 right) | 72.2k (2/2 right) |
| release-tags (long: 6+ calls) | 100.2k (2/2 right) | 73.3k (2/2 right) |
| docs-id (discovery, server not named) | 62.6k (2/2 right) | 50.3k (2/2 right) |
| deno-sections (one call, long answer) | 142.7k (2/2 right) | 50.5k (2/2 right) |
| memory-find (local server) | 65.5k (2/2 right) | 49.8k (2/2 right) |
| tokio-sections (two servers, a number) | 151.9k (2/2 right) | 78.2k (2/2 right) |
| axum-releases (big results, paging) | 74.1k (2/2 right) | 49.8k (2/2 right) |
