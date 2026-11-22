# Making dogu's code mode faster: research log

This page keeps every experiment on dogu's code mode, including the ones that lost, with the numbers and the files they came from. The plain-language results are in [BENCHMARKS.md](../../BENCHMARKS.md); this is the working record behind them.

## How the experiments are run

- **A second set of held-out tasks.** Before any change described here, 8 new tasks were written, committed and pushed ([997e64b](https://github.com/New1Direction/dogu/commit/997e64b)). They test things the first set did not: a count that moves during the run, sorting, a nested field, adding up, finding one section, two independent calls, counting in a local server against a random cut-off year, and comparing dates across calls. Every answer is fetched live or made up at random, and every check must pass "check the checks" before a run.
- **A/B in the same run.** The build under test (`codemode-b`) runs in the same session as the build it is compared with (`codemode`), task by task, so both meet the same API speed. Loading the servers directly runs alongside as the reference.
- **Turns in a fair order.** Since experiment 5, which setup goes first rotates from task to task (see "Mistakes in the bench" below).
- **A timeline for every run.** The bench notes when each event from Claude Code arrives, and splits every run into start-up, the model working, and tools running.
- **Model:** Claude Code with Sonnet. 2 or 3 runs per task and setup.

## Two facts about Claude Code that shape everything

**1. Claude Code cuts every MCP tool description, and a server's instructions, at 4,096 characters.** Measured by giving a test server a description and instructions made of numbered words (`w0001 w0002 …`) and asking which word came last: both stopped at character 4,096, followed by `… [truncated]`. Anything a code-mode server wants the model to see before it starts must fit in that space. (An earlier estimate of "about 2 KB" came from asking the model where a long list ended; models misreport that, so it was wrong.)

**2. Claude Code's own run time (`duration_ms`) leaves out start-up.** Loading the servers directly, Claude Code connects to every server before the task begins: 1.2 s in our runs, which `duration_ms` does not count. dogu connects later, so its connecting falls inside the task. Both views are fair, for different uses:

| | Per task, in a session that is already open | A one-off run, start-up included |
|---|---|---|
| Loading the servers directly | 4.10 s | 5.33 s |
| dogu code mode (0.5.0) | 4.51 s | 4.74 s |
| dogu folder | 5.52 s | 5.72 s |

From [2026-11-22-timeline](../../bench/results/2026-11-22-timeline.md), means of 16 runs each.

## The experiments

| # | Change | What happened | Kept? |
|---|---|---|---|
| 1 | Put every tool's full signature in `run_code`'s description (12.7 KB) | Cost +36%, still 3 turns. Claude Code cut the list at 4,096 characters, so the model saw a third of the tools and searched anyway. | No |
| 2 | Run the calls a program starts together at the same time (and connect to servers together) | A program making 4 calls: 2.42 s → **1.04 s**; 1 call: 0.95 s → 0.72 s (real servers, median of 7). In agent runs: no measurable change, because agents wrote `Promise.all` in 2 of 16 runs and most tasks need one call. | Yes |
| 3 | List every tool with its required arguments in `run_code`'s description, grouped by arguments (1,976 characters for 51 tools); a failed call's error shows the tool's full type | Turns 3 → **2** (median), input tokens −29%, cost −11%, 16/16 right. The slow runs guessed what a tool returns, got it wrong, and ran a second program. | Yes |
| 4 | Also list what each tool returns (3,984 characters) | No change: the same number of second programs, same time, same cost. A paper found return shapes matter most for weaker models. | No |
| 5 | `dogu serve` connects to every server as it starts, as Claude Code does with its own servers | The memory server's tool time: 0.7 s → **0.02 s**. With experiments 2 and 3, code mode now matches loading directly per task (median 3.4 s against 3.3 s) and is fastest as a one-off run (3.64 s against 4.64 s), at 63% lower cost, 21/21 right. A first run looked much slower; it was the run order (see below). | Yes |
| – | Reuse HTTP connections: read each event stream to its end (dogu stopped at its answer, so every call opened a new TLS connection), and open the connection during warm-up (with the newest protocol, "connecting" sent nothing) | GitHub calls on an open connection: 597 → **403 ms** (GitHub alone: 390-460 ms). A test server that streams like GitHub's: 5 calls used 8 connections before, 1 now. | Yes |
| 6 | Show two short examples of `Promise.all` in `run_code`'s description, with placeholder names | Judged on set 3, frozen before it: tool time 1.48 → **0.99 s**, per task 5.32 → 4.64 s (median, with the connection fixes). | Yes |
| 7 | Say to print a whole result when its shape is unknown, not a slice (dogu shortens long output itself) | Judged on set 4, frozen before it: see below. The extra turns came from looking at the first 500 characters of a result and running the call again. | Yes |
| 8 | Put the first sentence of what each tool does in `search`'s description, and tell the agent to check it before picking a tool | Judged on set 5: right answers 18 → 19 of 21, but median time 4.5 → **6.5 s** (one more turn) and cost +15%. The instruction made the agent stop and look things up, and the first sentence missed what mattered. | No |
| 9 | The same list, with any sentence that limits a tool ("only", "scoped", "excluding", "except") first, and no instruction | Judged on set 6, frozen before it: right answers **17 → 21 of 21** (loading directly: 21), median time 5.93 → 5.05 s, cost −2%. The agent never called `search`: it read "Already scoped to is:issue" in the list and chose another way. | Yes |
| 10 | A warm folder: one background dogu per folder keeps its servers running, and folder commands hand their calls to it | Folder commands after the first: npx memory server 0.69 → **0.01 s**, GitHub 1.00 → **0.46 s**. With Claude Code on set 3 (8 tasks × 3 runs, 24/24 right in each setup): per task 5.82 → **4.43 s** median; with start-up 6.01 → 4.62 s, against 4.56 s loading directly; cost $0.011 against $0.030. In an open session loading directly is still faster (3.38 s). The background process carries over from task to task, as it would while a person works. | Yes |
| – | Codex: when npm's cache (`~/.npm`) cannot be written, give npm one in the temp folder | Without the setting, 0.5.0 failed first and Codex fixed it itself from dogu's hint (up to 3 commands); now it works the first time. | Yes |

Where experiment 5 stands, from the fair-order run ([2026-11-22-exp5-warm-rotated](../../bench/results/2026-11-22-exp5-warm-rotated.md), 7 tasks × 3 runs, the Context7 task left out):

| | Loading directly | Code mode 0.5.0 | Code mode 0.6.0 |
|---|---|---|---|
| Per task in an open session, median | 3.3 s | 4.3 s | **3.4 s** |
| One-off run with start-up, median | 4.64 s | 4.48 s | **3.64 s** |
| One-off run with start-up, mean | **4.92 s** | 4.97 s | 5.28 s |
| Cost per task, mean | $0.0303 | $0.0124 | **$0.0111** |

The mean is higher than the median because of a few runs where the agent chose a slow way: paging through every open pull request instead of searching, or fetching releases one at a time in a loop.

Results files: [exp1](../../bench/results/2026-11-22-exp1-signatures.md), [exp2](../../bench/results/2026-11-22-exp2-parallel.md), [exp3](../../bench/results/2026-11-22-exp3-required-args.md), [exp4](../../bench/results/2026-11-22-exp4-returns.md), [exp5](../../bench/results/2026-11-22-exp5-warm.md), [Codex npm](../../bench/results/2026-11-22-codex-no-npm-setting-new.md).

## Where it stands after experiment 7

Held-out set 4 ([2026-11-22-exp7-whole](../../bench/results/2026-11-22-exp7-whole.md)), frozen before experiment 7 and never looked at while tuning; 7 tasks × 3 runs:

| | Per task, open session (median) | With start-up (median) | With start-up (mean) | Model turns | Cost | Right |
|---|---|---|---|---|---|---|
| Loading the servers directly | **3.50 s** | 4.70 s | 5.50 s | 2.29 | $0.025 | 21/21 |
| dogu folder | 5.11 s | 5.30 s | 5.95 s | 2.10 | $0.012 | 21/21 |
| dogu code mode 0.6.0 | 3.81 s | 4.03 s | 5.15 s | 2.33 | **$0.011** | 18/21 |
| dogu code mode, 0.6.0 + experiments 6 and 7 | 3.58 s | **3.82 s** | **4.87 s** | 2.29 | **$0.011** | 19/21 |

In an open session, code mode now ties loading the servers directly; counting start-up it is the fastest; it costs 54% less. The model turns are now the same. What is left is small: code mode's first turn is quicker (1.59 s against 1.83 s, it has less to read), its last turn a little slower (1.54 s against 1.44 s, it reads a whole result), and it writes about 20 more tokens.

**The wrong answers are real, and they show a cost of the short tool list.** Asked for astral-sh/ruff's open issues with pull requests included (2,336), code mode used GitHub's `search_issues` tool and answered 1,752, which counts issues only. Loading the servers directly, with every tool's full description in view, the agent used the repository lookup instead. Code mode's list names the tools and their arguments but not what each one does, and a name can mislead. (We first took 1,752 for GitHub's own search count and began to accept it; checking GitHub showed both of its counts at 2,336, so the check was right and stays as it was.)

**The fix for the wrong answers (experiments 8 and 9).** GitHub's `search_issues` is a semantic search over issues only; its description says "Already scoped to is:issue", but in its third sentence. Counting "open issues, pull requests included" with it gives the issues alone (pallets/click: 74, where GitHub counts 83 = 74 issues + 9 pull requests). Sets 5 and 6 were written for this kind of choice, each frozen before the change it judged. Experiment 9 brought code mode to the same 21 of 21 as loading the servers directly.

**The other held-out sets on the same code:** set 1 re-run with 0.6.0 ([2026-11-22-heldout-v0.6.0](../../bench/results/2026-11-22-heldout-v0.6.0.md), 7 of 8 tasks, 84/84 right): with start-up, code mode 5.06 s median against 5.56 s loading directly; in an open session 4.86 s against 4.33 s. That was before experiments 6 and 7.

## Where the time goes now

The model's own work is the biggest part of every task: 3.1 to 4.1 s per task on average, depending on the setup. What differs:

- **Start-up:** loading directly connects to everything first (1.2 s); dogu starts in 0.25 s.
- **Tools:** in the same message, Claude can ask for several tools and they run together. In code mode, an agent that writes a loop with `await` runs the calls one after another: 3.5 s against 1.1 s on the "newest release" task. Experiment 2 made calls started together run together, but agents seldom write them that way.

## Mistakes in the bench, found and fixed

- **A check that rejected right answers:** "Eight entities" was marked wrong because the check wanted the digit. Fixed, and the saved answers re-scored with `bench/rescore.ts` (the results file records it).
- **The order of runs:** in every A/B, the build under test ran last on each task, right after the others had used the same GitHub search endpoints. In the first run of experiment 5 that build had tool calls that stalled for 10 to 17 seconds, which we could not reproduce on their own; with the order rotating, the stalls were gone. GitHub rate limits landing on whoever went last are the likely cause, though not proven. The order rotates from now on.
- **Two "wrong checks" that were right:** twice an answer looked right and the check looked too strict (ruff, then pallets/click). Both times GitHub's own numbers showed the answer was wrong. A check is changed only after the live source has been checked.
- **Tuning on what you test:** looking closely at a set's programs to find the next change makes that set a poor judge of the change. So set 3 was frozen before experiment 6 and set 4 before experiment 7, each committed and pushed before the change was made ([6cfe245](https://github.com/New1Direction/dogu/commit/6cfe245), [0f244ea](https://github.com/New1Direction/dogu/commit/0f244ea)).
- **A quota:** Context7's free monthly quota ran out mid-run; every setup failed that task alike and said so. Runs after that leave the Context7 task out until there is a key.
- **Start-up time:** see "Two facts" above.

## Other code modes

Checked on 2026-11-22 against their own sources; numbers marked *vendor* are the maker's own claims.

| Name | What the model sees first | Runs locally with a CLI agent |
|---|---|---|
| [Cloudflare Code Mode](https://blog.cloudflare.com/code-mode/) | Generated TypeScript API; later `search` and `execute` | Partly |
| [Anthropic programmatic tool calling](https://platform.claude.com/docs/en/agents-and-tools/tool-use/programmatic-tool-calling) | Full tool definitions; Python | No (API feature) |
| [pi `codemode`](https://github.com/earendil-works/pi) | TypeScript declarations within a budget, plus search | Yes |
| [UTCP code-mode](https://github.com/universal-tool-calling-protocol/code-mode) | `search_tools`, `tool_info`, `call_tool_chain` | Yes |
| Codex `code_mode` | `exec`, `ALL_TOOLS` (in development) | Yes, behind a flag |

What others have published, as checked:

- Anthropic's docs: on τ²-bench, where each turn makes one or two calls in sequence, programmatic tool calling "left scores unchanged and cost roughly 8 percent more"; on a 75-tool benchmark it cut billed input tokens by about 38%.
- [Pydantic](https://pydantic.dev/articles/why-code-mode-is-the-future) (*vendor*, internal benchmark, 5,276 runs): long tasks cost 58% less and took 51% less time; single operations cost 8% more and took 16% longer (5 tests).
- [arXiv 2611.19123](https://arxiv.org/abs/2611.19123): declaring return shapes in code-mode stubs swung success by up to 39.5 points for lower-performing agents.
- Cloudflare's posts give no task-level numbers; the API post claims a 99.9% cut in tool-definition tokens.

The same pattern runs through all of them, and through ours: code mode saves a lot when a task makes many calls or handles large results, and breaks even or costs a little more on one quick call.

## Not done yet

- **UTCP code-mode** in the same bench. The setup is written (`utcp`, version 1.2.1 pinned) but not run: it would hand the GitHub token to a third-party package, which needs the owner's OK.
- **A multi-step task set**, written and frozen before any tuning for it, reported next to the quick tasks, not instead of them.
- **The folder's tool list** in the compact form from experiment 3 (about 2 KB instead of 18 KB).
- ~~A warm folder~~: done in experiment 10.
- ~~Short descriptions in code mode's list~~: done in experiment 9.
