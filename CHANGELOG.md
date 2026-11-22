# Changelog

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
