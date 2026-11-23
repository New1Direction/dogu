# MCPMark filesystem tasks: dogu code mode against the server loaded directly

MCPMark cd45b7f (eval-sys/mcpmark, Apache-2.0), 6 filesystem tasks, 3 runs each, Claude Code with sonnet. Right means MCPMark's own verify.py passed. Server: @modelcontextprotocol/server-filesystem@2025.12.18. Claude Code's own file and shell tools off in both setups.



| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| dogu serve (code mode) | 10/18 | 5.7k | 34.6k | $0.0485 | 13.4s | 4 | 0 |

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| dogu serve (code mode) | 0.28s | 13.08s | 0.09s | 4.3 |

| Task | codemode |
|---|---|
| easy/file_context/file_splitting | 0/3 |
| standard/desktop_template/contact_information | 2/3 |
| standard/file_context/uppercase | 1/3 |
| standard/legal_document/solution_tracing | 1/3 |
| standard/student_database/english_talent | 3/3 |
| standard/threestudio/requirements_completion | 3/3 |
