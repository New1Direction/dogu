# Benchmark 2026-11-22

dogu 0.3.0, Claude Code 2.1.330 (Claude Code), model `sonnet`, 3 runs of each task per setup, bench at commit 134678e. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).

## Held-out tasks (written before any run, never tuned against)



| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| Codex, dogu folder in AGENTS.md | 3/3 | n/a | 47.5k | n/a | 8.2s | 2 | 0 |

Median total input tokens per task:

| Task | Codex, dogu folder in AGENTS.md |
|---|---|
| memory-find (local server) | 47.5k (3/3 right) |
