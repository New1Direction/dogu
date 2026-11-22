# Changelog

## [0.7.0] - 2026-11-22

### Added

- A warm folder: the first folder command starts one background dogu for the folder, which keeps its servers running and connected, so later commands skip starting a server and connecting. Local npx server: 0.69 -> 0.01 s per call; GitHub: 1.00 -> 0.46 s. It uses a socket only you can use, steps aside on a new sync or changed settings, and leaves after `DOGU_WARM_IDLE` seconds (600). `DOGU_WARM=0` turns it off.

## [0.6.1] - 2026-11-22

### Fixed

- Code mode picks the right tool where a name misleads. `search`'s description now says what each tool does, with any sentence that limits a tool first: GitHub's `search_issues` "Already scoped to is:issue" searches issues only, so it cannot count issues and pull requests together. On new held-out tasks written for this: 17 -> 21 right out of 21, the same as loading the servers directly.
- Calls to HTTP servers reuse one connection. dogu stopped reading each streamed answer at the answer itself, so every call opened a new TCP and TLS connection; a GitHub call on an open connection went from about 600 to 400 ms.
- `dogu serve`'s warm start opens the network connection of modern HTTP servers too; before, connecting to them sent nothing, so the first call still paid for it.

### Changed

- `run_code`'s description shows how to start calls together with `Promise.all`, and says to print a whole result rather than a slice when its shape is unknown (dogu shortens long output itself). Both cut model turns and tool time on new held-out tasks.

## [0.6.0] - 2026-11-22

### Changed

- Code mode in Claude Code usually needs one model turn fewer. `run_code`'s description lists every tool with the arguments it requires, grouped by those arguments, within the 4,096 characters Claude Code shows of a description. On the second set of held-out tasks: typical task 4.5 s → 3.6 s with start-up, input tokens -30%, cost -10%, every answer right.
- `dogu serve` connects to every server as it starts, while the model reads the task, as Claude Code does with its own servers. It never opens a login for this; a server that fails is connected on first use, as before.
- Calls a program starts together (`Promise.all`) run at the same time, and the servers a program needs are connected at the same time: a program making 4 calls went from 2.4 s to 1.0 s.

### Added

- A failed tool call's error ends with the tool's full type, so the next try can get the arguments right.
- BENCHMARKS.md has pictures: the headline numbers, how each way works, and where the seconds of a task go. [docs/research/code-mode.md](docs/research/code-mode.md) records every experiment, including the ones that lost.
- The benchmark has a second set of held-out tasks, records when each step of a run happens, runs two builds against each other in one session, and rotates which setup goes first.

### Fixed

- Servers started with npx work inside Codex's sandbox without the `~/.npm` setting: when npm's cache cannot be written, dogu gives npm one in the temp folder.

## [0.5.0] - 2026-11-22

### Changed

- The sandbox runs on djs, a JavaScript and TypeScript engine in Rust built into dogu, so Deno is no longer needed. It blocks the same 17 escapes, starts 6 times faster and runs programs that call tools about 1.6 times faster; long computation loops are about 2 times slower. `DOGU_SANDBOX=deno` still uses Deno.

### Added

- [BENCHMARKS.md](BENCHMARKS.md): the results in plain words, with charts drawn from the result files.
- The benchmark covers Codex in its fast mode, and checks each task's answer check before any run.
- The README and `--help` show what each tool may change, as its server says: `[read-only]`, `[changes things]` or `[may delete or overwrite]`.
- `dogu sync --read-only` keeps only the tools servers mark read-only, and remembers it; `--all-tools` undoes it. Left-out tools cannot be called.

## [0.4.0] - 2026-11-22

### Added

- Forge: `./forge <name> program.ts`, or the `forge` tool in code mode, saves a program as a new tool, `forged/<name>`. It has its own flags and `--help`, appears in the README and in search, runs in the sandbox, and may only call the tools its code names. Measured: once forged, a repeated job costs 28 to 31% less and is faster.
- The folder is an Agent Skill: `dogu sync` writes `SKILL.md`, and `--link-skill` links the folder into `~/.claude/skills` and `~/.agents/skills`.
- The benchmark covers Claude Code (with and without its tool search), Codex and pi, on 8 held-out tasks written before any run.

### Changed

- The README's tool list drops detail in steps, descriptions first and names last, within a 32 KB budget.
- `run_code` in `dogu serve` cuts outputs over 30,000 characters in the middle, with a note.

### Fixed

- A local server that dies says why: dogu shows its last lines, with a hint when npm cannot write `~/.npm`, as inside Codex's sandbox. Before, agents saw only "broken pipe".

## [0.3.0] - 2026-11-22

Faster and cheaper for agents. Measured with the new benchmark in `bench/`: with GitHub, Context7 and DeepWiki, both the folder and code mode cost about half as much per task as loading the servers directly, with every answer right.

### Added

- The folder's README lists every tool with its flags and the fields it returns, so an agent can run the right command straight away. Over 16 KB it lists tool names only.
- `--json` prints a tool's result as one line of JSON, so `jq` always works.
- A tool's `--help` shows the fields it returns.
- `bench/`: a benchmark written with Effect that has Claude Code do the same tasks with the tools given four ways, and checks every answer.

### Changed

- Code mode's `search` lists every app and tool in its description, ranks tools by how many words match instead of needing all of them, and returns each match's TypeScript type, so one search is enough.
- `dogu sync` suggests putting the tool list in your agent's standing instructions, which saves a step on every task.

### Fixed

- Searching code mode by a tool's exact name, such as `search_repositories`, found nothing.

## [0.2.1] - 2026-11-22

### Added

- Prebuilt binaries for macOS (Apple Silicon and Intel) and Linux (x86_64 and arm64), and an install script, so you no longer need Rust.
- `dogu sync` finds Claude Code's `~/.claude.json`. It takes your own servers plus the ones `claude mcp add` saved for the folder you are in.
- `--help` works on every command.
- A tool's `--help` shows a JSON example made from its real parameters.

### Changed

- Running `dogu sync` again uses the config the folder was made from. Pass `--config` to switch.
- An unknown command exits with an error instead of printing the usage.

### Tested

- Built and run on macOS.
- Linux and macOS CI, the MCP conformance suite (35 client scenarios) and the sandbox escape test run on every push.
- Linear, with a real OAuth login and token refresh, and DeepWiki.

## [0.2.0] - 2026-11-22

First release: the folder of commands, the sandbox, `dogu serve`, OAuth login, and both MCP protocol eras.
