# MCPMark filesystem tasks: dogu code mode against the server loaded directly

MCPMark 2dd8a97 (eval-sys/mcpmark, Apache-2.0), 14 filesystem tasks, 3 runs each, Claude Code with sonnet. Right means MCPMark's own verify.py passed. Server: @modelcontextprotocol/server-filesystem@2025.12.18. Claude Code's own file and shell tools off in both setups.

- dogu serve (code mode): 41% fewer input tokens, 35% lower cost and 41% less time than mcp servers loaded directly.
- dogu code mode, candidate build: 3% more input tokens, 4% higher cost and 2% more time than dogu serve (code mode).
- dogu code mode, candidate build: 39% fewer input tokens, 32% lower cost and 39% less time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 41/42 | 6.5k | 30.0k | $0.0371 | 11.3s | 4 | 0 |
| dogu serve (code mode) | 42/42 | 5.6k | 17.7k | $0.0243 | 6.7s | 3 | 0 |
| dogu code mode, candidate build | 42/42 | 5.6k | 18.2k | $0.0252 | 6.9s | 3 | 0 |

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.00s | 12.92s | 0.06s | 4.9 |
| dogu serve (code mode) | 0.25s | 7.50s | 0.05s | 3.1 |
| dogu code mode, candidate build | 0.25s | 8.08s | 0.05s | 3.2 |

| Task | direct | codemode | codemode-b |
|---|---|---|---|
| heldout11/json/bump_version | 3/3 | 3/3 | 3/3 |
| heldout11/search/find_word | 3/3 | 3/3 | 3/3 |
| heldout11/text/lowercase_notes | 2/3 | 3/3 | 3/3 |
| heldout11/text/merge_parts | 3/3 | 3/3 | 3/3 |
| heldout11/text/number_lines | 3/3 | 3/3 | 3/3 |
| heldout11/text/reverse_lines | 3/3 | 3/3 | 3/3 |
| heldout11/text/word_counts | 3/3 | 3/3 | 3/3 |
| heldout12/code/todo_report | 3/3 | 3/3 | 3/3 |
| heldout12/config/find_setting | 3/3 | 3/3 | 3/3 |
| heldout12/data/sales_total | 3/3 | 3/3 | 3/3 |
| heldout12/data/unique_emails | 3/3 | 3/3 | 3/3 |
| heldout12/docs/doc_index | 3/3 | 3/3 | 3/3 |
| heldout12/files/sort_inbox | 3/3 | 3/3 | 3/3 |
| heldout12/logs/error_count | 3/3 | 3/3 | 3/3 |
