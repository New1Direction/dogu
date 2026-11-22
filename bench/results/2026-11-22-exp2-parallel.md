# Benchmark 2026-11-22

dogu 0.5.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 2 runs of each task per setup, bench at commit 4afce84. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 2 (frozen before the code-mode speed work)

- dogu serve (code mode): 74% fewer input tokens, 56% lower cost and 26% more time than mcp servers loaded directly.
- dogu code mode, candidate build: 2% more input tokens, 6% higher cost and 10% more time than dogu serve (code mode).
- dogu code mode, candidate build: 73% fewer input tokens, 53% lower cost and 39% more time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 16/16 | 29.0k | 59.3k | $0.0300 | 3.8s | 2 | 0 |
| dogu serve (code mode) | 16/16 | 4.6k | 15.7k | $0.0133 | 4.7s | 3 | 0 |
| dogu code mode, candidate build | 16/16 | 4.6k | 16.1k | $0.0140 | 5.2s | 3 | 0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|---|
| open-prs (a count) | 58.1k (2/2 right) | 17.7k (2/2 right) | 17.7k (2/2 right) |
| oldest-issue (sorting) | 58.3k (2/2 right) | 15.8k (2/2 right) | 15.9k (2/2 right) |
| release-author (a nested field) | 59.2k (2/2 right) | 12.4k (2/2 right) | 13.7k (2/2 right) |
| stars-sum (adding up) | 59.9k (2/2 right) | 15.6k (2/2 right) | 15.7k (2/2 right) |
| deepwiki-node (finding one section) | 59.4k (2/2 right) | 13.4k (2/2 right) | 13.5k (2/2 right) |
| two-libraries (two independent calls) | 59.5k (2/2 right) | 14.6k (2/2 right) | 18.8k (2/2 right) |
| memory-years (counting in a local server) | 58.5k (2/2 right) | 12.6k (2/2 right) | 9.9k (2/2 right) |
| newest-release (comparing across calls) | 99.8k (2/2 right) | 16.3k (2/2 right) | 16.4k (2/2 right) |
