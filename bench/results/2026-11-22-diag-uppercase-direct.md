# MCPMark filesystem tasks: dogu code mode against the server loaded directly

MCPMark cd45b7f (eval-sys/mcpmark, Apache-2.0), 1 filesystem tasks, 3 runs each, Claude Code with sonnet. Right means MCPMark's own verify.py passed. Server: @modelcontextprotocol/server-filesystem@2025.12.18. Claude Code's own file and shell tools off in both setups.



| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 0/3 | 6.5k | 25.6k | $0.0526 | 14.5s | 8 | 0 |

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 1.69s | 14.43s | 0.10s | 8.0 |

| Task | direct |
|---|---|
| easy/file_context/uppercase | 0/3 |
