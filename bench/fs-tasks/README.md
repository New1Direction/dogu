# Held-out filesystem tasks (set 11)

Seven tasks in [MCPMark](https://github.com/eval-sys/mcpmark)'s layout, so `bench/mcpmark.ts` runs them:
a prompt (`description.md`), a starting folder (`states/workspace`, named in `meta.json` as
`local:workspace`) and a checker (`verify.py`, standard library only).

They copy, join and rewrite file contents, where a program that reads a result the wrong way writes
the wrong bytes. They were written and committed before experiment 15 (telling code-mode agents
what tool results look like), to judge it; see docs/research/code-mode.md.

    python3 bench/fs-tasks/check_checks.py          # each checker rejects the start, accepts a right answer
    MCPMARK_DIR=bench/fs-tasks deno run -A bench/mcpmark.ts
