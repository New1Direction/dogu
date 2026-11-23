# Benchmark 2026-11-23

dogu 0.7.2, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit d6c816b with uncommitted changes. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)

- dogu serve (code mode): 25% fewer input tokens, 29% lower cost and 2% less time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 32% more input tokens, 44% higher cost and 53% more time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 75% more input tokens, 103% higher cost and 56% more time than dogu serve (code mode).

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 6/6 | 7.0k | 14.8k | $0.0144 | 3.0s | 2 | 0 |
| dogu serve (code mode) | 6/6 | 5.2k | 11.2k | $0.0102 | 2.9s | 2 | 0 |
| UTCP code-mode 1.2.1 | 6/6 | 4.5k | 19.6k | $0.0207 | 4.6s | 3 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.12s | 3.36s | 0.12s | 2.0 |
| dogu serve (code mode) | 0.27s | 2.80s | 0.14s | 2.0 |
| UTCP code-mode 1.2.1 | 2.53s | 4.56s | 0.12s | 3.0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | UTCP code-mode 1.2.1 |
|---|---|---|---|
| deno-sections (one call, long answer) | 15.5k (3/3 right) | 11.8k (3/3 right) | 20.2k (3/3 right) |
| memory-find (local server) | 14.2k (3/3 right) | 10.5k (3/3 right) | 18.9k (3/3 right) |

## Held-out tasks, set 2 (frozen before the code-mode speed work)

- dogu serve (code mode): 24% fewer input tokens, 27% lower cost and 3% less time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 31% more input tokens, 33% higher cost and 61% more time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 72% more input tokens, 81% higher cost and 67% more time than dogu serve (code mode).

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 6/6 | 7.0k | 15.1k | $0.0149 | 2.8s | 2 | 0 |
| dogu serve (code mode) | 6/6 | 5.2k | 11.4k | $0.0109 | 2.7s | 2 | 0 |
| UTCP code-mode 1.2.1 | 6/6 | 4.5k | 19.7k | $0.0198 | 4.5s | 3 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.00s | 3.58s | 0.11s | 2.0 |
| dogu serve (code mode) | 0.26s | 2.80s | 0.10s | 2.0 |
| UTCP code-mode 1.2.1 | 2.36s | 4.90s | 0.13s | 3.0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | UTCP code-mode 1.2.1 |
|---|---|---|---|
| deepwiki-node (finding one section) | 15.5k (3/3 right) | 11.8k (3/3 right) | 20.2k (3/3 right) |
| memory-years (counting in a local server) | 14.6k (3/3 right) | 11.0k (3/3 right) | 19.1k (3/3 right) |

## Held-out tasks, set 3 (frozen before the work on calls made together)

- dogu serve (code mode): 23% fewer input tokens, 22% lower cost and 7% less time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 33% more input tokens, 46% higher cost and 33% more time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 73% more input tokens, 88% higher cost and 43% more time than dogu serve (code mode).

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 6/6 | 7.0k | 14.9k | $0.0144 | 2.7s | 2 | 0 |
| dogu serve (code mode) | 6/6 | 5.2k | 11.5k | $0.0112 | 2.5s | 2 | 0 |
| UTCP code-mode 1.2.1 | 6/6 | 4.5k | 19.9k | $0.0211 | 3.6s | 3 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 0.98s | 3.03s | 0.13s | 2.0 |
| dogu serve (code mode) | 0.27s | 2.70s | 0.12s | 2.0 |
| UTCP code-mode 1.2.1 | 2.43s | 3.52s | 0.14s | 3.0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | UTCP code-mode 1.2.1 |
|---|---|---|---|
| bun-section (finding one section) | 15.7k (3/3 right) | 12.0k (3/3 right) | 20.4k (3/3 right) |
| started-in (a lookup in a local server) | 14.1k (3/3 right) | 10.9k (3/3 right) | 18.9k (3/3 right) |

## Held-out tasks, set 4 (frozen before the change about printing whole results)

- dogu serve (code mode): 25% fewer input tokens, 30% lower cost and 4% less time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 32% more input tokens, 47% higher cost and 65% more time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 76% more input tokens, 109% higher cost and 71% more time than dogu serve (code mode).

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 6/6 | 7.0k | 14.7k | $0.0136 | 2.6s | 2 | 0 |
| dogu serve (code mode) | 6/6 | 5.2k | 11.1k | $0.0096 | 2.5s | 2 | 0 |
| UTCP code-mode 1.2.1 | 6/6 | 4.5k | 19.5k | $0.0200 | 4.3s | 3 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 0.99s | 2.50s | 0.10s | 2.0 |
| dogu serve (code mode) | 0.29s | 2.40s | 0.11s | 2.0 |
| UTCP code-mode 1.2.1 | 2.37s | 4.38s | 0.11s | 3.0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | UTCP code-mode 1.2.1 |
|---|---|---|---|
| ruff-section (finding one section) | 15.3k (3/3 right) | 11.6k (3/3 right) | 20.0k (3/3 right) |
| entity-year (a lookup in a local server) | 14.2k (3/3 right) | 10.5k (3/3 right) | 18.9k (3/3 right) |

## Held-out tasks, set 5 (frozen before the accuracy fix)

- dogu serve (code mode): 25% fewer input tokens, 32% lower cost and 5% less time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 32% more input tokens, 48% higher cost and 47% more time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 77% more input tokens, 117% higher cost and 55% more time than dogu serve (code mode).

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 3/3 | 7.0k | 14.6k | $0.0132 | 2.9s | 2 | 0 |
| dogu serve (code mode) | 3/3 | 5.2k | 11.0k | $0.0090 | 2.8s | 2 | 0 |
| UTCP code-mode 1.2.1 | 3/3 | 4.5k | 19.4k | $0.0195 | 4.3s | 3 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.00s | 3.51s | 0.27s | 2.0 |
| dogu serve (code mode) | 0.25s | 2.41s | 0.26s | 2.0 |
| UTCP code-mode 1.2.1 | 2.40s | 4.93s | 0.29s | 3.0 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | UTCP code-mode 1.2.1 |
|---|---|---|---|
| click-testing-section (finding one section) | 14.6k (3/3 right) | 11.0k (3/3 right) | 19.4k (3/3 right) |

## Multi-step tasks (frozen before any run with them)

- dogu serve (code mode): 23% fewer input tokens, 21% lower cost and 7% less time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 53% more input tokens, 41% higher cost and 77% more time than mcp servers loaded directly.
- UTCP code-mode 1.2.1: 98% more input tokens, 78% higher cost and 91% more time than dogu serve (code mode).

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 3/6 | 7.0k | 16.5k | $0.0213 | 3.6s | 3 | 0 |
| dogu serve (code mode) | 3/6 | 5.2k | 12.7k | $0.0168 | 3.3s | 2 | 0 |
| UTCP code-mode 1.2.1 | 4/6 | 4.6k | 25.1k | $0.0300 | 6.4s | 4 | 0 |

Where the time went:

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.01s | 3.47s | 0.22s | 2.3 |
| dogu serve (code mode) | 0.29s | 3.01s | 0.28s | 2.0 |
| UTCP code-mode 1.2.1 | 2.49s | 6.67s | 0.61s | 4.2 |

Median total input tokens per task:

| Task | MCP servers loaded directly | dogu serve (code mode) | UTCP code-mode 1.2.1 |
|---|---|---|---|
| most-sections (3 calls, compare) | 18.3k (0/3 right) | 14.5k (0/3 right) | 30.4k (1/3 right) |
| odd-years (filtering in a local server) | 14.6k (3/3 right) | 11.0k (3/3 right) | 19.4k (3/3 right) |
