# Benchmark 2026-11-22

dogu 0.5.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 2 runs of each task per setup, bench at commit f7e90f1 with uncommitted changes. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks, set 2 (frozen before the code-mode speed work)



| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| Codex, dogu folder in AGENTS.md | 2/2 | n/a | 97.4k | n/a | 20.1s | 4 | 0 |

Median total input tokens per task:

| Task | Codex, dogu folder in AGENTS.md |
|---|---|
| memory-years (counting in a local server) | 97.4k (2/2 right) |
