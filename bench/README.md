# Benchmark

The same tasks, done by Claude Code with the same three hosted servers (GitHub's official server, Context7 and DeepWiki), given to it three ways:

| Setup | What the agent gets |
|---|---|
| `direct` | The three servers in its MCP config, so all 51 tool definitions are in its context. No other tools. |
| `folder` | Only Bash and Read, and one line: "Your tools are in `<folder>`. Read `<folder>/README.md` first." |
| `codemode` | Only `dogu serve`: two tools, `search` and `run_code`. |

Each run starts in an empty folder with `--setting-sources project`, so your own settings, hooks and plugins stay out of it.

## Tasks

| Task | What it needs |
|---|---|
| `lookup` | One call: the latest release tag of `modelcontextprotocol/typescript-sdk`. |
| `search` | One call, filtered: the 3 most-starred repos in the `modelcontextprotocol` org. |
| `chain` | Two servers: GitHub to find the top repo, DeepWiki for its doc sections. |
| `docs` | Context7: the library ID for Effect. |
| `compare` | Several calls: which of the top 3 repos has the most open issues. |

The right answers are fetched live at the start of each benchmark run, from the GitHub API and the servers themselves, and every answer is checked against them (`tasks.ts`). A run that answers wrong still counts in the averages.

## What is measured

From Claude Code's `stream-json` output, per run:

- **First-turn tokens**: what the model reads before it does anything: system prompt, tool definitions and the task.
- **Total input tokens**: everything the model read over the run, cached or not, counted once per model message.
- **Cost**: Claude Code's `total_cost_usd`, at API prices. Prompt caching makes re-reading the same context cheap, so cost falls less than tokens do.
- **Time**, **turns**, and whether the agent **went around its tools** (for example `curl` to the GitHub API).

## Run it

You need `claude` and `gh` logged in, `dogu` on your PATH, and [Deno](https://deno.com). The harness is written with [Effect](https://effect.website), which Deno loads from npm; there is nothing to install.

```
deno run --allow-run --allow-env --allow-read --allow-write --allow-net bench/run.ts
```

| Variable | Default | |
|---|---|---|
| `BENCH_REPS` | 3 | Runs of each task per setup |
| `BENCH_CONCURRENCY` | 3 | Runs at the same time |
| `BENCH_MODEL` | `sonnet` | Passed to `claude --model` |
| `BENCH_TASKS` | all | A comma list, e.g. `lookup,chain` |

A full run is 45 Claude Code sessions. Results go to `bench/results/<date>.md` and `.json`. The JSON has every run, including the answer text.

Your GitHub token (from `gh auth token`) is written only to a temp folder that only you can read, and that folder is deleted when the run ends, even if it fails.

## Limits

- One model, one client (Claude Code), three servers. Other agents and bigger tool sets will differ. The gap grows with the number of tools, since the folder's README grows by one line per server and the definitions by every tool.
- Hosted servers vary in speed from minute to minute. Setups are interleaved so a slow minute hits all of them.
- Each run is a fresh session. In a long session the direct setup's tool definitions stay cached, which helps its cost but not its context size.

Tests for the harness itself: `deno test bench/bench_test.ts`.
