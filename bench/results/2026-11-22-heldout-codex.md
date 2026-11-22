# Benchmark 2026-11-22

dogu 0.3.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 2 runs of each task per setup, bench at commit f3268c5 (the Codex client is unchanged since 084d9d1). Codex's default model on a ChatGPT plan. Codex's sandbox did not allow ~/.npm here, so the local memory server failed inside it; see 2026-11-22-codex-memory-npm for the same task with ~/.npm allowed (3/3). Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- Codex, dogu folder in AGENTS.md: 6% fewer input tokens and 2% less time than codex, mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| Codex, MCP servers loaded directly | 16/16 | n/a | 68.0k | n/a | 12.6s | 2 | 6 |
| Codex, dogu folder in AGENTS.md | 15/16 | n/a | 64.1k | n/a | 12.4s | 2.5 | 0 |

Median total input tokens per task:

| Task | Codex, MCP servers loaded directly | Codex, dogu folder in AGENTS.md |
|---|---|---|
| created (one call, exact) | 63.2k (2/2 right) | 49.1k (2/2 right) |
| issue-title (one call, exact) | 61.4k (2/2 right) | 76.6k (2/2 right) |
| release-tags (long: 6+ calls) | 101.1k (2/2 right) | 77.6k (2/2 right) |
| docs-id (discovery, server not named) | 64.9k (2/2 right) | 47.8k (2/2 right) |
| deno-sections (one call, long answer) | 138.4k (2/2 right) | 51.6k (2/2 right) |
| memory-find (local server) | 65.2k (2/2 right) | 134.4k (1/2 right) |
| tokio-sections (two servers, a number) | 177.3k (2/2 right) | 77.0k (2/2 right) |
| axum-releases (big results, paging) | 68.0k (2/2 right) | 47.4k (2/2 right) |
