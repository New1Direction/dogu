# Benchmark 2026-11-22

dogu 0.4.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 1 run of each task per setup, bench at commit 4092181. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)



| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| dogu serve (code mode) | 8/8 | 4.6k | 15.3k | $0.0124 | 4.4s | 3 | 0 |

Median total input tokens per task:

| Task | dogu serve (code mode) |
|---|---|
| created (one call, exact) | 9.6k (1/1 right) |
| issue-title (one call, exact) | 15.7k (1/1 right) |
| release-tags (long: 6+ calls) | 16.3k (1/1 right) |
| docs-id (discovery, server not named) | 18.2k (1/1 right) |
| deno-sections (one call, long answer) | 10.7k (1/1 right) |
| memory-find (local server) | 9.4k (1/1 right) |
| tokio-sections (two servers, a number) | 16.6k (1/1 right) |
| axum-releases (big results, paging) | 14.9k (1/1 right) |
