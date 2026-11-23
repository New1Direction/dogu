# MCPMark filesystem tasks: dogu code mode against the server loaded directly

MCPMark dbee20e (eval-sys/mcpmark, Apache-2.0), 7 filesystem tasks, 3 runs each, Claude Code with sonnet. Right means MCPMark's own verify.py passed. Server: @modelcontextprotocol/server-filesystem@2025.12.18. Claude Code's own file and shell tools off in both setups.

- dogu serve (code mode): 39% fewer input tokens, 33% lower cost and 37% less time than mcp servers loaded directly.
- dogu code mode, candidate build: 2% fewer input tokens, 4% lower cost and 14% less time than dogu serve (code mode).
- dogu code mode, candidate build: 40% fewer input tokens, 35% lower cost and 46% less time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 19/21 | 6.5k | 29.7k | $0.0366 | 10.8s | 4 | 0 |
| dogu serve (code mode) | 21/21 | 5.5k | 18.0k | $0.0246 | 6.8s | 3 | 0 |
| dogu code mode, candidate build | 21/21 | 5.6k | 17.7k | $0.0237 | 5.8s | 3 | 0 |

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 0.99s | 11.60s | 0.06s | 4.7 |
| dogu serve (code mode) | 0.23s | 6.86s | 0.04s | 3.3 |
| dogu code mode, candidate build | 0.23s | 6.16s | 0.04s | 3.0 |

| Task | direct | codemode | codemode-b |
|---|---|---|---|
| heldout11/json/bump_version | 3/3 | 3/3 | 3/3 |
| heldout11/search/find_word | 3/3 | 3/3 | 3/3 |
| heldout11/text/lowercase_notes | 2/3 | 3/3 | 3/3 |
| heldout11/text/merge_parts | 2/3 | 3/3 | 3/3 |
| heldout11/text/number_lines | 3/3 | 3/3 | 3/3 |
| heldout11/text/reverse_lines | 3/3 | 3/3 | 3/3 |
| heldout11/text/word_counts | 3/3 | 3/3 | 3/3 |
