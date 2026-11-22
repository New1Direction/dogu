# Changelog

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
