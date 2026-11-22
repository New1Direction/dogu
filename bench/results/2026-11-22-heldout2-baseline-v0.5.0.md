# Benchmark 2026-11-22

dogu 0.5.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 2 runs of each task per setup, bench at commit 997e64b. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

> Re-scored: the memory-years check first rejected numbers written as words ("Eight entities"), which were right. With the fixed check, all 64 runs are right (`bench/rescore.ts`; the answers are unchanged).

## Held-out tasks, set 2 (frozen before the code-mode speed work)

- dogu folder, list in instructions: 56% fewer input tokens, 58% lower cost and 31% more time than mcp servers loaded directly.
- dogu folder, list in instructions: 11% more input tokens, 62% lower cost and 9% less time than loaded directly, claude code tool search on.
- dogu serve (code mode): 74% fewer input tokens, 57% lower cost and 31% more time than mcp servers loaded directly.
- dogu serve (code mode): 33% fewer input tokens, 61% lower cost and 9% less time than loaded directly, claude code tool search on.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 16/16 | 29.0k | 59.3k | $0.0301 | 3.9s | 2 | 0 |
| Loaded directly, Claude Code tool search on | 16/16 | 7.0k | 23.3k | $0.0333 | 5.6s | 3 | 0 |
| dogu folder, list in instructions | 16/16 | 12.7k | 25.9k | $0.0126 | 5.1s | 2 | 0 |
| dogu serve (code mode) | 16/16 | 4.6k | 15.6k | $0.0129 | 5.1s | 3 | 0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | Loaded directly, Claude Code tool search on | dogu folder, list in instructions | dogu serve (code mode) |
|---|---|---|---|---|
| open-prs (a count) | 58.1k (2/2 right) | 22.9k (2/2 right) | 25.6k (2/2 right) | 17.7k (2/2 right) |
| oldest-issue (sorting) | 58.4k (2/2 right) | 24.3k (2/2 right) | 25.8k (2/2 right) | 15.8k (2/2 right) |
| release-author (a nested field) | 59.2k (2/2 right) | 22.9k (2/2 right) | 25.7k (2/2 right) | 12.4k (2/2 right) |
| stars-sum (adding up) | 59.9k (2/2 right) | 24.2k (2/2 right) | 25.7k (2/2 right) | 15.6k (2/2 right) |
| deepwiki-node (finding one section) | 59.4k (2/2 right) | 23.0k (2/2 right) | 26.9k (2/2 right) | 10.7k (2/2 right) |
| two-libraries (two independent calls) | 59.5k (2/2 right) | 24.9k (2/2 right) | 26.4k (2/2 right) | 14.7k (2/2 right) |
| memory-years (counting in a local server) | 58.5k (2/2 right) | 22.0k (1/2 right) | 26.0k (2/2 right) | 9.9k (0/2 right) |
| newest-release (comparing across calls) | 99.8k (2/2 right) | 43.3k (2/2 right) | 39.3k (2/2 right) | 16.3k (2/2 right) |
