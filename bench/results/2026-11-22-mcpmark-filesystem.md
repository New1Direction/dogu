# MCPMark filesystem tasks: dogu code mode against the server loaded directly

MCPMark cd45b7f (eval-sys/mcpmark, Apache-2.0), 40 filesystem tasks, 3 runs each, Claude Code with sonnet. Right means MCPMark's own verify.py passed. Server: @modelcontextprotocol/server-filesystem@2025.12.18. Claude Code's own file and shell tools off in both setups.

- dogu serve (code mode): 38% fewer input tokens, 74% lower cost and 51% less time than mcp servers loaded directly.

| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |
|---|---|---|---|---|---|---|---|
| MCP servers loaded directly | 102/120 | 6.6k | 51.9k | $0.1991 | 24.3s | 7 | 0 |
| dogu serve (code mode) | 101/120 | 5.7k | 31.9k | $0.0515 | 11.8s | 4 | 0 |

| Setup | Starting up (mean) | Model working (mean) | Tools running (mean) | Model turns (mean) |
|---|---|---|---|---|
| MCP servers loaded directly | 0.98s | 46.79s | 0.25s | 13.8 |
| dogu serve (code mode) | 0.24s | 12.91s | 0.10s | 4.2 |

| Task | direct | codemode |
|---|---|---|
| easy/file_context/file_splitting | 3/3 | 1/3 |
| easy/file_context/pattern_matching | 3/3 | 3/3 |
| easy/file_context/uppercase | 0/3 | 0/3 |
| easy/file_property/largest_rename | 3/3 | 3/3 |
| easy/file_property/txt_merging | 3/3 | 3/3 |
| easy/folder_structure/structure_analysis | 3/3 | 3/3 |
| easy/legal_document/file_reorganize | 3/3 | 3/3 |
| easy/papers/papers_counting | 3/3 | 3/3 |
| easy/student_database/duplicate_name | 3/3 | 3/3 |
| easy/student_database/recommender_name | 3/3 | 3/3 |
| standard/desktop/music_report | 3/3 | 3/3 |
| standard/desktop/project_management | 3/3 | 3/3 |
| standard/desktop/timeline_extraction | 1/3 | 2/3 |
| standard/desktop_template/budget_computation | 0/3 | 0/3 |
| standard/desktop_template/contact_information | 3/3 | 2/3 |
| standard/desktop_template/file_arrangement | 1/3 | 1/3 |
| standard/file_context/duplicates_searching | 3/3 | 3/3 |
| standard/file_context/file_merging | 3/3 | 3/3 |
| standard/file_context/file_splitting | 3/3 | 3/3 |
| standard/file_context/pattern_matching | 1/3 | 3/3 |
| standard/file_context/uppercase | 3/3 | 1/3 |
| standard/file_property/size_classification | 3/3 | 3/3 |
| standard/file_property/time_classification | 3/3 | 3/3 |
| standard/folder_structure/structure_analysis | 3/3 | 3/3 |
| standard/folder_structure/structure_mirror | 2/3 | 3/3 |
| standard/legal_document/dispute_review | 3/3 | 3/3 |
| standard/legal_document/individual_comments | 3/3 | 3/3 |
| standard/legal_document/solution_tracing | 3/3 | 0/3 |
| standard/papers/author_folders | 0/3 | 3/3 |
| standard/papers/find_math_paper | 3/3 | 3/3 |
| standard/papers/organize_legacy_papers | 3/3 | 3/3 |
| standard/student_database/duplicate_name | 3/3 | 3/3 |
| standard/student_database/english_talent | 3/3 | 2/3 |
| standard/student_database/gradebased_score | 3/3 | 3/3 |
| standard/threestudio/code_locating | 3/3 | 3/3 |
| standard/threestudio/output_analysis | 1/3 | 3/3 |
| standard/threestudio/requirements_completion | 3/3 | 2/3 |
| standard/votenet/dataset_comparison | 3/3 | 3/3 |
| standard/votenet/debugging | 3/3 | 3/3 |
| standard/votenet/requirements_writing | 3/3 | 3/3 |
