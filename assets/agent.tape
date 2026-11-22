# A real agent using the folder: Claude Code (Sonnet), run headless, told only where the tools are.
# Each ▸ line is a command it ran, shown as it runs. Recorded with VHS (https://github.com/charmbracelet/vhs).
#
#   T=$(gh auth token)
#   HOME=/Users/Shared/dogu-demo GITHUB_TOKEN=$T dogu sync     # same three servers as demo.tape
#   GITHUB_TOKEN=$T vhs assets/agent.tape

Output assets/agent.gif
Output assets/agent.mp4

Require claude
Require dogu
Require jq

Set Shell "bash"
Set FontSize 17
Set FontFamily "JetBrains Mono"
Set Width 1280
Set Height 820
Set Padding 24
Set LineHeight 1.25
Set TypingSpeed 18ms
Set CursorBlink false
Set WindowBar Colorful
Set WindowBarSize 36
Set BorderRadius 12
Set Theme { "name": "dogu", "background": "#0B1020", "foreground": "#F6F1E4", "cursor": "#D9A441", "selection": "#17264D", "black": "#101B3A", "red": "#E8776C", "green": "#8FD19E", "yellow": "#D9A441", "blue": "#6C8BE8", "magenta": "#B48EE8", "cyan": "#7FC8E8", "white": "#B9C4E4", "brightBlack": "#7384B8", "brightRed": "#F09A90", "brightGreen": "#B2E3BD", "brightYellow": "#ECC274", "brightBlue": "#9DAED8", "brightMagenta": "#CDB2F0", "brightCyan": "#A8DCF0", "brightWhite": "#F6F1E4" }

Hide
Type `source assets/agent.sh && cd /Users/Shared/dogu-demo && export PS1='\[\e[38;2;217;164;65m\]$\[\e[0m\] ' && clear`
Enter
Sleep 500ms
Show

Type `agent "Your tools are in /Users/Shared/dogu-demo/mcp. Read its README.md first.`
Enter
Type `Which 3 modelcontextprotocol repos have the most GitHub stars, and what are the main doc`
Enter
Type `sections of the top one? Answer in under 8 lines."`
Sleep 500ms
Enter
Wait+Screen@90s /turns, [0-9]+s/
Sleep 5s
