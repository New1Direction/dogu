# Benchmark

The same tasks, done by Claude Code with the same three hosted servers (GitHub's official server, Context7 and DeepWiki), given to it three ways:

| Setup | What the agent gets |
|---|---|
| `direct` | The three servers in its MCP config, so all 51 tool definitions are in its context. No other tools. |
| `folder` | Only Bash and Read, with the folder's README in its standing instructions: what `@~/mcp/README.md` in CLAUDE.md, or pasting the README into AGENTS.md, gives. Passed with `--append-system-prompt`, because headless Claude Code does not load an untrusted folder's CLAUDE.md. |
| `folder-read` | Only Bash and Read, and one line in the task: "Your tools are in `<folder>`. Read `<folder>/README.md` first." Works with any agent. |
| `codemode` | Only `dogu serve`: two tools, `search` and `run_code`. |

Runs use `--setting-sources project`, so your own settings, hooks and plugins stay out of it.

## How the runs are set up

Prompt caching decides most of the cost, so the runs are set up to cache the way real use does:

- **Every run of a setup works in the same empty folder**, like a person working in one project. The standing part of each session (Claude Code's system prompt, the tool definitions, the README) is then the same from run to run and stays cached, as it does between a person's sessions.
- **Every prompt starts with a unique run ID**, so no run can replay an earlier conversation from the cache.
- **One warm-up session per setup** runs first and is not counted, so every setup is measured with its standing part already cached.

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
- **Cost**: Claude Code's `total_cost_usd`, at API prices, with input split into fresh, written to the cache, and read from it. Writing to the cache costs most; reading costs least.
- **Time**, **turns**, the **steps** (every command or tool call, in order), and whether the agent **went around its tools** (`curl`, `wget`, or the GitHub CLI).

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
| `BENCH_SETUPS` | all | A comma list, e.g. `folder,codemode` |
| `BENCH_OUT` | the date | Name of the results files |

A full run is 60 Claude Code sessions plus 4 warm-ups. Results go to `bench/results/<date>.md` and `.json`. The JSON has every run, including the answer text.

Your GitHub token (from `gh auth token`) is written only to a temp folder that only you can read, and that folder is deleted when the run ends, even if it fails.

## Limits

- One model, one client (Claude Code), three servers. Other agents and bigger tool sets will differ. The gap grows with the number of tools, since the folder's README grows by one line per server and the definitions by every tool.
- Hosted servers vary in speed from minute to minute. Setups are interleaved so a slow minute hits all of them.
- The tasks are short, one to three tool calls. In a long session the direct setup re-reads all 51 definitions on every turn, so its context and cost grow faster than these numbers show.

Tests for the harness itself: `deno test bench/bench_test.ts`.
