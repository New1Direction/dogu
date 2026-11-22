# The README demo, recorded with VHS (https://github.com/charmbracelet/vhs) against real servers.
#
#   DEMO_HOME=<folder with .config/dogu/mcp.json> GITHUB_TOKEN=$(gh auth token) vhs assets/demo.tape
#
# The config lists three hosted servers:
#   github   https://api.githubcopilot.com/mcp/   with "Authorization": "Bearer ${GITHUB_TOKEN}"
#   context7 https://mcp.context7.com/mcp
#   deepwiki https://mcp.deepwiki.com/mcp

Output assets/demo.gif
Output assets/demo.mp4

Require dogu
Require jq
Require deno

Set Shell "bash"
Set FontSize 17
Set FontFamily "JetBrains Mono"
Set Width 1280
Set Height 760
Set Padding 24
Set LineHeight 1.25
Set TypingSpeed 32ms
Set CursorBlink false
Set WindowBar Colorful
Set WindowBarSize 36
Set BorderRadius 12
Set Margin 0
Set Theme { "name": "dogu", "background": "#0B1020", "foreground": "#F6F1E4", "cursor": "#D9A441", "selection": "#17264D", "black": "#101B3A", "red": "#E8776C", "green": "#8FD19E", "yellow": "#D9A441", "blue": "#6C8BE8", "magenta": "#B48EE8", "cyan": "#7FC8E8", "white": "#B9C4E4", "brightBlack": "#7384B8", "brightRed": "#F09A90", "brightGreen": "#B2E3BD", "brightYellow": "#ECC274", "brightBlue": "#9DAED8", "brightMagenta": "#CDB2F0", "brightCyan": "#A8DCF0", "brightWhite": "#F6F1E4" }

Hide
Type `export HOME="$DEMO_HOME" PS1='\[\e[38;2;217;164;65m\]$\[\e[0m\] ' && cd ~ && rm -rf ~/mcp && clear`
Enter
Sleep 500ms
Show

# 1. One command turns every server you have into a folder of commands.
Type "dogu sync"
Sleep 400ms
Enter
Sleep 4.5s

# 2. They are files.
Type "cd ~/mcp && ls . deepwiki"
Sleep 300ms
Enter
Sleep 3s

# 3. The agent learns a tool only when it needs it. No network.
Type "deepwiki/ask_wiki_question --help"
Sleep 300ms
Enter
Sleep 4s

Type "clear"
Enter

# 4. Real calls, piped like any other command.
Type `github/search_repositories --query org:modelcontextprotocol --sort stars | jq -r '.items[:5][].full_name'`
Sleep 400ms
Enter
Sleep 3.5s

# 5. Many calls in one sandboxed program. Results go tool to tool, not through the model.
Type "./run - <<'EOF'"
Enter
Type `const q = { query: "org:modelcontextprotocol", sort: "stars", perPage: 3 };`
Enter
Type "const top = await tools.github.search_repositories(q);"
Enter
Type "for (const repo of top.items) {"
Enter
Type "  const wiki = await tools.deepwiki.read_wiki_structure({ repoName: repo.full_name });"
Enter
Type `  console.log(repo.full_name.padEnd(36), wiki.result.split("\n- ").length - 1, "doc sections");`
Enter
Type "}"
Enter
Type "EOF"
Sleep 400ms
Enter
Sleep 5s
