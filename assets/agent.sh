# Used by assets/agent.tape. Runs Claude Code headless and shows each command it runs, as it runs it, then its answer.
agent() {
  claude -p "$1" --model sonnet --setting-sources project --strict-mcp-config --mcp-config '{"mcpServers":{}}' \
    --allowedTools Bash Read --output-format stream-json --verbose 2>/dev/null |
  jq -r --unbuffered '
    if .type == "assistant" then
      .message.content[]
      | if .type == "tool_use" then "\u001b[38;2;217;164;65m  ▸ \u001b[0m" + ((.input.command // ("read " + (.input.file_path // ""))) | gsub("\n"; " ") | .[0:104])
        else empty end
    elif .type == "result" then "\n" + (.result | gsub("\\*\\*|`"; "")) + "\n\u001b[38;2;115;132;184m  " + (.num_turns|tostring) + " turns, " + ((.duration_ms/1000)|floor|tostring) + "s\u001b[0m"
    else empty end'
}
