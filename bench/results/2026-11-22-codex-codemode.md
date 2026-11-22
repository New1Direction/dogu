# Benchmark 2026-11-22

dogu 0.7.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 2 runs of each task per setup, bench at commit 6bcffd1. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 3 (frozen before the work on calls made together)

- Codex, dogu folder in AGENTS.md: 26% fewer input tokens and 25% less time than codex, mcp servers loaded directly.
- Codex, its own code mode: 1% more input tokens and 3% more time than codex, mcp servers loaded directly.
- Codex, dogu code mode (dogu serve): 8% more input tokens and 13% less time than codex, mcp servers loaded directly.
- Codex, dogu code mode (dogu serve): 7% more input tokens and 16% less time than codex, its own code mode.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| Codex, MCP servers loaded directly | 16/16 | n/a | 64.8k | n/a | 11.9s | 2 | 3 |
| Codex, dogu folder in AGENTS.md | 16/16 | n/a | 48.2k | n/a | 9.0s | 2 | 0 |
| Codex, its own code mode | 16/16 | n/a | 65.3k | n/a | 12.2s | 2 | 2 |
| Codex, dogu code mode (dogu serve) | 16/16 | n/a | 69.8k | n/a | 10.3s | 2 | 8 |

Where the time went:



Median total input tokens per task:

| Task | Codex, MCP servers loaded directly | Codex, dogu folder in AGENTS.md | Codex, its own code mode | Codex, dogu code mode (dogu serve) |
|---|---|---|---|---|
| most-stars (fan-out: 4 calls) | 63.6k (2/2 right) | 49.9k (2/2 right) | 83.8k (2/2 right) | 89.9k (2/2 right) |
| three-tags (fan-out: 3 calls) | 63.8k (2/2 right) | 48.2k (2/2 right) | 61.1k (2/2 right) | 53.4k (2/2 right) |
| big-repos (a count) | 88.0k (2/2 right) | 48.5k (2/2 right) | 89.5k (2/2 right) | 87.9k (2/2 right) |
| first-pr (one call) | 62.4k (2/2 right) | 49.8k (2/2 right) | 65.3k (2/2 right) | 102.6k (2/2 right) |
| bun-section (finding one section) | 70.4k (2/2 right) | 50.4k (2/2 right) | 67.8k (2/2 right) | 45.7k (2/2 right) |
| started-in (a lookup in a local server) | 64.6k (2/2 right) | 48.0k (2/2 right) | 67.3k (2/2 right) | 91.7k (2/2 right) |
| newer-sdk (fan-out: 2 calls, compare) | 65.1k (2/2 right) | 50.0k (2/2 right) | 62.4k (2/2 right) | 41.9k (2/2 right) |
| most-issues (fan-out after a search) | 53.2k (2/2 right) | 48.2k (2/2 right) | 62.0k (2/2 right) | 69.2k (2/2 right) |
