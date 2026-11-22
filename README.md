# dogu

Your MCP tools as a folder of commands, and as a sandboxed code API.

Each app is a folder. Each tool is a small program inside it. An AI agent (or you) finds tools with `ls` and runs them like any other command. The agent reads the instructions for a tool only when it needs that tool, so 500 connected tools cost the same context as 5.

```
~/mcp/
  README.md            how to use the folder (the agent reads this first)
  github/
    README.md          the tools, one line each
    create_issue       run it; --help shows the parameters
    search_repos
    index.d.ts         the same tools as TypeScript types (for code mode)
  run                  runs a TypeScript program in the sandbox (code mode)
```

## Install

```
cargo install --path .
```

## Use

```
dogu sync
```

That reads your MCP config, asks each server for its tools and writes `~/mcp`. Then tell your agent:

> Your tools are in ~/mcp. Read ~/mcp/README.md first.

Run `dogu sync` again when your servers change.

`dogu sync` looks for the config in this order: `--config <file>`, `$DOGU_CONFIG`, `./.mcp.json`, `./mcp.json`, `~/.config/dogu/mcp.json`, the Claude Desktop config, `~/.cursor/mcp.json`. Use `--out <folder>` to write somewhere other than `~/mcp`.

## Running a tool

```
github/create_issue --help                           # parameters, from a saved copy (no network)
github/create_issue --repo me/app --title "Bug"      # flags
github/create_issue '{"repo": "me/app", "title": "Bug"}'   # JSON
cat issue.json | github/create_issue -               # JSON from a pipe
files/read_multiple_files --paths a.txt --paths b.txt       # repeat a flag for a list
```

- Results print to stdout. Errors print to stderr with exit code 1.
- Images and audio are saved to a temp file and the path is printed.
- `--raw` prints the server's full JSON answer.
- Because tools are commands, they chain: `github/search_repos --query rust | jq ... | head`.

## Config

The usual `mcp.json`:

```json
{
  "mcpServers": {
    "files":  { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/Users/me/Documents"] },
    "remote": { "url": "https://example.com/mcp", "headers": { "Authorization": "Bearer ${MY_TOKEN}" } }
  }
}
```

`${NAME}` is replaced with that environment variable. If a remote server has no `Authorization` header in the config, dogu uses the login flow below when the server asks for one. Local servers run in the folder the config file is in, unless the server has a `"cwd"`.

## Logging in to remote servers

Many hosted servers (GitHub, Linear, Notion and others) want an OAuth login. If `dogu sync` says a server needs a login:

```
dogu login          # opens your browser for each server that needs it
dogu sync
```

- Logins are saved in `~/.config/dogu/auth.json`, readable only by you. Tokens are refreshed when they expire.
- `dogu login <server>` logs in to one server. `dogu logout <server>` forgets it.
- Tools never open a browser by themselves. If a login has run out, the tool fails and says `Run: dogu login <server>`.
- On a machine with no browser, set `DOGU_NO_BROWSER=1` and open the printed link on another machine. The link comes back to `127.0.0.1:38573` on the machine that runs dogu, so forward that port (`ssh -L 38573:127.0.0.1:38573 ...`).

How dogu gets a client ID, in order:

1. From your config, if you registered an app with the service yourself: `"oauth": { "client_id": "...", "client_secret": "..." }` next to the `"url"`.
2. From a Client ID Metadata Document, if you host one. Put `client-metadata.json` (in this repo) on an HTTPS URL you own, fix the URLs inside it, and set `DOGU_CLIENT_ID_URL` to that URL. This is the way the current spec prefers.
3. By registering itself with the service (Dynamic Client Registration). This needs no setup and works with most servers today, but the spec has deprecated it.

## Protocol versions

MCP changed shape in the 2026-07-28 release: the `initialize` handshake and sessions are gone, and every request says which version it speaks. dogu speaks both the new protocol and the older one. It tries the new one first and falls back, and it remembers the answer per server so later calls skip the check.

## Code mode

For a job with many steps, the agent writes one TypeScript program and dogu runs it in a sandbox. The program cannot read files, use the network, read your environment or start other programs. The only thing it can reach is your tools.

```
~/mcp/run - <<'EOF'
const issues = await tools.github.search_issues({ query: "is:open label:bug" });
const old = issues.filter((i) => i.comments === 0);
console.log(old.length, "open bugs with no reply");
EOF
```

- `tools.<app>.<tool>({...})` returns a Promise. A failed call throws an Error.
- The result is the tool's structured data if it has any, else parsed JSON, else text.
- In code, `-` and `.` in names become `_`: the tool `get-sum` is `tools.app.get_sum`.
- Each app folder has an `index.d.ts` with the types. The agent reads it before writing.
- Calls to the same server reuse one connection, so only the first is slow.

Code mode needs [Deno](https://deno.com) installed (`brew install deno`). Deno is the sandbox.

Limits:

| Limit | Default | Change with |
|---|---|---|
| Which tools the program may call | all | `--allow github --allow 'files/read_*'` |
| Run time | 120 seconds | `--timeout 30` or `DOGU_TIMEOUT` |
| Memory | 256 MB | `DOGU_MEMORY_MB` |
| Printed output | 1 MB | `DOGU_MAX_OUTPUT` |

Every tool call is recorded in `~/mcp/.dogu/log.jsonl`: time, tool, worked or not, how long. Parameters and results are not recorded.

### For apps with no shell

`dogu serve` is an MCP server with two tools: `search` (find tools, read their types) and `run_code` (run a program in the sandbox). Add it to Claude Desktop or any MCP app, and all your other servers sit behind those two tools.

1. Move your real servers into `~/.config/dogu/mcp.json`.
2. Run `dogu sync --config ~/.config/dogu/mcp.json`.
3. In the app's own config, keep only this:

```json
{ "mcpServers": { "tools": { "command": "dogu", "args": ["serve"] } } }
```

### What the sandbox does and does not protect

- **It does** stop a program from touching your files, network, environment and processes. The test program `tests/escape.ts` tries 17 ways out, and all are blocked.
- **It does not** make your tools safe. If a tool can delete files or send email, a program can call it. Use `--allow` to give a program only what the job needs.
- **It only matters if the agent has no other way out.** An agent with a shell can skip the sandbox. For the full safety benefit, give the agent `dogu serve` and no shell.
- **It is Deno's permission system, not a virtual machine.** Deno maintains it as a security boundary, but a bug in Deno would be a bug here.

## Tested against

`tests/conformance.sh` runs the official MCP conformance suite (`@modelcontextprotocol/conformance`) against dogu. 35 client scenarios pass: 8 protocol ones (old handshake, new per-request metadata, HTTP headers, schema handling) and the 27 core OAuth ones (discovery, PKCE, scopes, step-up, issuer checks, token endpoint auth, registration). Not passing: the scenarios for elicitation, SSE resume, and the OAuth extensions (client credentials, DPoP, enterprise login), which dogu does not implement.

`tests/escape.ts` is the sandbox escape test for code mode.

## Not built yet

- **Tools that ask the user a question mid-call** (elicitation, sampling). dogu says it cannot do these, so servers should not ask. If one does, the call fails with a clear message.
- **The old SSE transport** (2024-11-05) and SSE stream resume. HTTP servers must speak Streamable HTTP.
- **Windows.** The stubs are `sh` scripts.
- **Warm servers in folder mode.** Each stub call starts the local server, runs the tool and stops it. Servers started with `npx` take a second or more per call. (`serve` already keeps connections open, and a program keeps them open while it runs.)
- **Resources and prompts.** Tools only for now.
- **Type checking before a run.** Types are stripped, not checked, so a wrong parameter shows up as an error from the tool.
- **Token storage in the system keychain.** Tokens are in a file only you can read.

## How it works

About 3,100 lines of Rust and JavaScript:

| File | Job |
|---|---|
| `src/config.rs` | read `mcp.json` |
| `src/client.rs` | talk MCP over stdio or HTTP, in both protocol eras |
| `src/auth.rs` | OAuth login: discovery, browser round trip, tokens, refresh |
| `src/sync.rs` | write the folder: stubs, READMEs, TypeScript types |
| `src/call.rs` | folder mode: run one tool from the command line |
| `src/run.rs` | code mode: start the sandbox, answer the program's tool calls |
| `src/runner.js` | the only trusted code inside the sandbox |
| `src/serve.rs` | be an MCP server with `search` and `run_code` |
| `src/conformance.rs` | the small command the conformance suite runs |
| `src/util.rs` | base64, SHA-256, random bytes |
