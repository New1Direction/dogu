# Benchmark 2026-11-22

dogu 0.5.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 2 runs of each task per setup, bench at commit d894b0b with uncommitted changes. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 2 (frozen before the code-mode speed work)

- dogu serve (code mode): 74% fewer input tokens, 56% lower cost and 2% less time than mcp servers loaded directly.
- dogu code mode, candidate build: 29% fewer input tokens, 11% lower cost and 9% less time than dogu serve (code mode).
- dogu code mode, candidate build: 81% fewer input tokens, 61% lower cost and 12% less time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 16/16 | 29.0k | 59.3k | $0.0302 | 4.9s | 2 | 0 |
| dogu serve (code mode) | 16/16 | 4.6k | 15.6k | $0.0133 | 4.7s | 3 | 0 |
| dogu code mode, candidate build | 16/16 | 5.0k | 11.1k | $0.0118 | 4.3s | 2 | 0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | dogu code mode, candidate build |
|---|---|---|---|
| open-prs (a count) | 58.1k (2/2 right) | 18.5k (2/2 right) | 21.1k (2/2 right) |
| oldest-issue (sorting) | 58.3k (2/2 right) | 15.8k (2/2 right) | 10.5k (2/2 right) |
| release-author (a nested field) | 59.2k (2/2 right) | 12.4k (2/2 right) | 10.8k (2/2 right) |
| stars-sum (adding up) | 59.9k (2/2 right) | 15.6k (2/2 right) | 13.9k (2/2 right) |
| deepwiki-node (finding one section) | 59.4k (2/2 right) | 10.7k (2/2 right) | 11.5k (2/2 right) |
| two-libraries (two independent calls) | 59.5k (2/2 right) | 14.7k (2/2 right) | 11.4k (2/2 right) |
| memory-years (counting in a local server) | 58.5k (2/2 right) | 9.7k (2/2 right) | 10.6k (2/2 right) |
| newest-release (comparing across calls) | 99.8k (2/2 right) | 16.3k (2/2 right) | 13.7k (2/2 right) |
