# Benchmark 2026-11-22

dogu 0.5.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 2 runs of each task per setup, bench at commit 1f88582. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 2 (frozen before the code-mode speed work)

- dogu serve (code mode): 73% fewer input tokens, 55% lower cost and 27% more time than mcp servers loaded directly.
- dogu code mode, candidate build: 32% more input tokens, 36% higher cost and 8% more time than dogu serve (code mode).
- dogu code mode, candidate build: 65% fewer input tokens, 39% lower cost and 36% more time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 16/16 | 29.0k | 59.3k | $0.0300 | 3.9s | 2 | 0 |
| dogu serve (code mode) | 16/16 | 4.6k | 15.7k | $0.0135 | 5.0s | 3 | 0 |
| dogu code mode, candidate build | 16/16 | 5.7k | 20.7k | $0.0184 | 5.4s | 3 | 0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|---|
| open-prs (a count) | 58.1k (2/2 right) | 17.7k (2/2 right) | 23.8k (2/2 right) |
| oldest-issue (sorting) | 58.4k (2/2 right) | 15.9k (2/2 right) | 23.7k (2/2 right) |
| release-author (a nested field) | 59.2k (2/2 right) | 12.4k (2/2 right) | 12.3k (2/2 right) |
| stars-sum (adding up) | 59.9k (2/2 right) | 15.6k (2/2 right) | 26.9k (2/2 right) |
| deepwiki-node (finding one section) | 59.4k (2/2 right) | 10.7k (2/2 right) | 12.9k (2/2 right) |
| two-libraries (two independent calls) | 59.5k (2/2 right) | 18.7k (2/2 right) | 12.8k (2/2 right) |
| memory-years (counting in a local server) | 58.5k (2/2 right) | 9.9k (2/2 right) | 19.2k (2/2 right) |
| newest-release (comparing across calls) | 99.8k (2/2 right) | 16.3k (2/2 right) | 32.2k (2/2 right) |
