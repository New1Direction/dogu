# Benchmark 2026-11-23

dogu 0.7.2, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit 28c669a with uncommitted changes. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 10 (frozen before experiment 14)

- dogu folder, list in instructions: 71% fewer input tokens, 70% lower cost and 25% more time than mcp servers loaded directly.
- dogu folder, candidate build: 2% more input tokens, 5% lower cost and 29% less time than dogu folder, list in instructions.
- dogu folder, candidate build: 70% fewer input tokens, 71% lower cost and 11% less time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 21/21 | 27.2k | 84.9k | $0.0522 | 7.0s | 7 | 0 |
| dogu folder, list in instructions | 21/21 | 12.2k | 25.0k | $0.0157 | 8.8s | 2 | 0 |
| dogu folder, candidate build | 21/21 | 12.5k | 25.5k | $0.0150 | 6.3s | 2 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.24s | 4.79s | 1.29s | 4.0 |
| dogu folder, list in instructions | 0.25s | 5.37s | 3.20s | 2.4 |
| dogu folder, candidate build | 0.27s | 4.72s | 1.63s | 2.1 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu folder, list in instructions | dogu folder, candidate build |
|---|---|---|---|
| biome-tags (fan-out where some calls fail) | 102.9k (3/3 right) | 24.9k (3/3 right) | 25.4k (3/3 right) |
| pnpm-tags (fan-out where some calls fail) | 92.0k (3/3 right) | 25.0k (3/3 right) | 25.5k (3/3 right) |
| bevy-tags (fan-out where some calls fail) | 84.9k (3/3 right) | 25.1k (3/3 right) | 25.7k (3/3 right) |
| cli-tags (5 known calls) | 62.0k (3/3 right) | 24.9k (3/3 right) | 25.5k (3/3 right) |
| pnpm-newest (fan-out where some calls fail, compare) | 92.0k (3/3 right) | 37.8k (3/3 right) | 25.4k (3/3 right) |
| fd-issue-title (one call, big result) | 54.9k (3/3 right) | 37.6k (3/3 right) | 38.4k (3/3 right) |
| zed-created (one call, exact) | 55.0k (3/3 right) | 24.7k (3/3 right) | 25.2k (3/3 right) |
