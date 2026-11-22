<p align="center">
  <img src="assets/hero.svg" width="100%" alt="dogu fetches your tools. The logo is a wrench that is also a long dog: the open end is its head, the ring end its tail, two bolts its legs. Your MCP tools, as a folder of commands: a folder tree shows ~/mcp with github, gmail and files folders, each holding tools as runnable programs.">
</p>

<p align="center">
  <a href="https://github.com/New1Direction/dogu/actions/workflows/ci.yml"><img src="https://github.com/New1Direction/dogu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/New1Direction/dogu/releases/latest"><img src="https://img.shields.io/github/v/release/New1Direction/dogu" alt="Latest release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT license"></a>
</p>

**Code mode and a local folder of commands, for every MCP tool you have.**

dogu turns each tool into a small program in a folder. An agent uses them the way it uses any other command: `ls` to find one, `--help` to learn it, a pipe to chain two. For a job with many calls, it writes one TypeScript program and dogu runs it in a sandbox.

Measured with GitHub, Context7 and DeepWiki connected: either way costs about half as much per task as loading the servers into the agent directly, and gets the same answers. [The benchmark](bench/) is in the repo.

```
curl -fsSL https://raw.githubusercontent.com/New1Direction/dogu/main/install.sh | sh
dogu sync
```

<p align="center">
  <img src="assets/agent.gif" width="100%" alt="Claude Code, run headless, is told only: Your tools are in the mcp folder, read its README first. It reads the README, greps the github and deepwiki READMEs, reads search_repositories --help, searches GitHub for the modelcontextprotocol repos with the most stars, then reads the DeepWiki structure of the top one. It answers with servers, python-sdk and typescript-sdk and the six doc sections of servers, in 6 turns and 10 seconds.">
</p>

<p align="center"><sub>Claude Code, told only where the folder is. Recorded live: each ▸ is a command it ran. <a href="assets/agent.tape">How this was made</a>.</sub></p>

## The problem

Connect an agent to MCP servers and every tool's full definition is loaded into its context before you say a word. With a few servers that is thousands of tokens spent on tools the agent will mostly never call. It costs money on every message, and the more tools there are, the worse the agent gets at picking the right one.

dogu keeps the tools on disk instead. The agent gets one line per tool, with its flags, and looks up the details only for the tool it uses.

Measured, not estimated, on [tasks written before any run](bench/): GitHub's official server, Context7, DeepWiki and a local memory server, 3 runs each, every answer checked. Claude Code on Sonnet with dogu 0.7.0, 84 runs, all right (7 of the 8 tasks: the Context7 one was left out for every setup because its free quota had run out):

| | Read before starting | Read in total | Cost per task | Time per task | With start-up |
|---|---|---|---|---|---|
| The servers loaded directly | 29.0k tokens | 59.3k | $0.036 | 3.9 s | 5.4 s |
| The same, with Claude Code's tool search on | 7.0k | 23.0k | $0.038 | 6.0 s | 7.2 s |
| dogu folder, tool list in the agent's instructions | 12.6k | 26.8k | $0.013 | 5.3 s | 5.4 s |
| **dogu code mode (`dogu serve`)** | **6.7k** | **13.9k** | **$0.011** | **3.7 s** | **3.9 s** |

![Cost per task with Claude Code and dogu 0.7.0: loading the servers directly $0.036, with Claude Code's tool search $0.038, dogu folder $0.013, dogu code mode $0.011](docs/bench/cost.svg)

dogu's code mode costs 68% less than loading the servers directly and is also faster; the folder costs 63% less, and is about 1.4 s slower per task once a session is open (counting start-up, which Claude Code's own time leaves out, they tie). Tool search reads fewer tokens than loading everything, but costs more, because it brings tool definitions in mid-session, and those are cached at the higher write price. Times are medians. The cheapest setup measured is dogu's code mode with Haiku: $0.0010 per task, all answers right, 97% less than Sonnet with the servers loaded directly. On Haiku the order of cost is the same: code mode 57% less than loading directly, the folder 53% less. On tasks that fan out to many calls dogu halves the cost too, and code mode is faster when the agent starts the calls together; see [the benchmarks](BENCHMARKS.md#tasks-with-many-calls).

### Other agents

The gap depends on the agent:

- **Codex** (its default model): use the dogu folder in `AGENTS.md`. Against Codex's own code mode (still under development in Codex 0.171), it read 26% fewer tokens and was 26% faster, 9.0 s against 12.2 s, and never went around its tools; `dogu serve` works in Codex too, but Codex went around it with `curl` in half the runs. Against the servers loaded directly, the folder read 30% fewer tokens and was faster than Codex with the servers loaded directly, 11.2 s against 13.8 s, and in Codex's fast mode 9.2 s against 10.0 s. That was measured with `~/.npm` writable; before 0.6.0, without it a local server could not start and the saving was 6%. dogu now handles that itself. With the Terra model it read about half the tokens, and fast mode took it from 11.1 s to 7.3 s per task. With the servers loaded directly, Codex often skipped them and fetched pages with `curl` itself (11 of 32 runs); with the folder, 1 of 32.
- **pi**: pi has its own code mode, built for the same problem. With gpt-6.2-sol, dogu's code mode (`dogu serve` added to pi as an MCP server) beat it on two sets of tasks: 31% and 35% cheaper, 36% and 28% faster, the same answers right, and 73% cheaper than pi with the tools declared directly. pi's code mode searches for each tool first, which takes a turn; dogu's list already says what each tool needs. (The dogu folder, with gpt-6.1-sol, was the fastest in pi but cost 24% more than pi's code mode.)

**[The benchmarks in plain words, with charts](BENCHMARKS.md).** All results, and how to run them yourself: [`bench/`](bench/).

## What you get

- **A smaller context.** The agent reads about the tools it uses and no others.
- **Room for hundreds of tools.** A tool nobody calls costs nothing.
- **Big results stay out of the chat.** Send a result to a file or through `jq` and `head`. The agent sees only what it needs.
- **Tools chain.** The output of one goes into the next without passing through the model.
- **You can use them too.** They are normal commands. Run them in a terminal, a script or a cron job.
- **A sandbox for many-step jobs.** The agent writes one TypeScript program, and dogu runs it with no access to your files or network.

## Install

On macOS or Linux:

```
curl -fsSL https://raw.githubusercontent.com/New1Direction/dogu/main/install.sh | sh
```

It downloads the build for your machine from the latest release, checks its SHA-256, and puts `dogu` in `~/.local/bin`. Set `DOGU_INSTALL_DIR` to put it somewhere else.

Or build it with [Rust](https://rustup.rs):

```
cargo install --git https://github.com/New1Direction/dogu
```

Then:

```
dogu sync
```

`dogu sync` reads the MCP config you already have, asks each server for its tools, and writes them to `~/mcp`. It looks for a config in this order:

1. `--config <file>`
2. `$DOGU_CONFIG`
3. `./.mcp.json`, then `./mcp.json`
4. `~/.config/dogu/mcp.json`
5. Claude Code's `~/.claude.json`: your servers, plus the ones added for the folder you run `dogu sync` in
6. The Claude Desktop config
7. `~/.cursor/mcp.json`

It takes the first one that lists any servers.

Then tell your agent where the tools are. The fastest and cheapest way is to put the folder's README, which lists every tool, in its standing instructions. In Claude Code's `CLAUDE.md`:

> @~/mcp/README.md

For any other agent, one line in `AGENTS.md` or a system prompt works too, at the cost of one extra step per task:

> Your tools are in ~/mcp. Read ~/mcp/README.md first.

Or let agents find it on their own: the folder is also an [Agent Skill](https://agentskills.io). `dogu sync --link-skill` links it into `~/.claude/skills` and `~/.agents/skills`, where Claude Code, Codex, pi and others look.

**Codex users:** Codex's sandbox blocks npm's cache in `~/.npm`. Since 0.6.0 dogu notices that and gives npm a cache in the temp folder, so local servers that start with `npx` work with no setting. To let npm keep its own cache instead, allow it in `~/.codex/config.toml`:

```toml
[sandbox_workspace_write]
writable_roots = ["/Users/you/.npm"]
```

Run `dogu sync` again when your servers change. It reads the same config as the first time. To switch, pass `--config`.

## What the folder looks like

For example, with GitHub and a filesystem server connected:

```
~/mcp/
  README.md            how to use the folder. The agent reads this first.
  run                  runs a TypeScript program in the sandbox
  github/
    README.md          the tools, one line each
    create_issue       a program. --help shows its parameters.
    search_repos
    index.d.ts         the same tools as TypeScript types
  files/
    read_text_file
    ...
```

## Use a tool

<p align="center">
  <img src="assets/demo.gif" width="100%" alt="A terminal session with real hosted servers. dogu sync reads the config: context7 2 tools, deepwiki 3 tools, github 46 tools. ls shows the apps and the deepwiki tools as files. deepwiki/ask_wiki_question --help prints its parameters with no network call. github/search_repositories finds the modelcontextprotocol repos with the most stars and jq prints their names. A TypeScript program in the sandbox searches GitHub and reads each top repo's DeepWiki structure in one run.">
</p>

```sh
github/create_issue --help                                  # parameters, from a saved copy. No network.
github/create_issue --repo me/app --title "Bug"             # flags
github/create_issue '{"repo": "me/app", "title": "Bug"}'    # JSON
cat issue.json | github/create_issue -                      # JSON from a pipe
files/read_multiple_files --paths a.txt --paths b.txt       # repeat a flag for a list
github/search_repos --query rust | jq '.items[].name' | head
```

- Results print to stdout. Errors print to stderr with exit code 1.
- A missing parameter, a typo in a flag or a wrong type is caught before the server is called, with a message that says what to fix.
- Images and audio are saved to a temp file, and the path is printed.
- `--json` prints the result as one line of JSON, so `jq` always works. The README shows the fields each tool returns, like `→ {tag_name, html_url, …}`.
- `--raw` prints the server's full JSON answer.

## Three ways to use it

<p align="center">
  <img src="assets/modes.svg" width="100%" alt="Diagram. Your MCP servers connect to dogu, which connects, logs in, speaks old and new MCP, and logs every call. Three ways out: dogu sync gives a folder of commands, dogu run gives a sandbox for programs, dogu serve gives one MCP server with two tools.">
</p>

### 1. A folder of commands

This is everything above. It works with any agent that can run shell commands, and it needs nothing else installed.

The first command starts one background dogu for the folder, which keeps its servers running and connected, so later commands do not start a server or connect again: a local server started with npx answers in about 0.01 s instead of 0.7 s, and a GitHub call takes about 0.5 s instead of 1. It listens on a socket only you can use, steps aside if the folder is synced again or a setting such as a token changes, and leaves after 10 minutes without a call, closing the servers it started. `DOGU_WARM=0` turns it off. (Not on Windows.)

### 2. A sandbox for programs

When a job needs many calls, a loop, or filtering of big results, the agent writes one TypeScript program:

```sh
~/mcp/run - <<'EOF'
const issues = await tools.github.search_issues({ query: "is:open label:bug" });
const quiet = issues.filter((i) => i.comments === 0);
console.log(quiet.length, "open bugs with no reply");
EOF
```

- `tools.<app>.<tool>({...})` returns a Promise. A failed call throws an Error.
- The result is the tool's structured data if it has any, else parsed JSON, else text.
- In code, `-` and `.` in names become `_`: the tool `get-sum` is `tools.app.get_sum`.
- Each app folder has an `index.d.ts`. The agent reads it before it writes.
- Calls to one server reuse one connection, so only the first is slow.

The sandbox is [djs](https://github.com/earendil-works/djs), a JavaScript and TypeScript engine written in Rust and built into dogu, so there is nothing else to install. `DOGU_SANDBOX=deno` runs programs in [Deno](https://deno.com) instead, with every permission off.

| Limit | Default | Change with |
|---|---|---|
| Which tools the program may call | all | `--allow github --allow 'files/read_*'` |
| Run time | 120 seconds | `--timeout 30` or `DOGU_TIMEOUT` |
| Memory | 256 MB | `DOGU_MEMORY_MB` |
| Printed output | 1 MB | `DOGU_MAX_OUTPUT` |

### 3. One MCP server with three tools

Some apps have no shell, such as Claude Desktop. For those, `dogu serve` is an MCP server with three tools: `search` finds tools and returns their types, `run_code` runs a program in the sandbox, and `forge` saves a program as a new tool. All your other servers sit behind those three.

1. Move your real servers into `~/.config/dogu/mcp.json`.
2. Run `dogu sync --config ~/.config/dogu/mcp.json`.
3. In the app's own config, keep only this:

```json
{ "mcpServers": { "tools": { "command": "dogu", "args": ["serve"] } } }
```

## Make your own tools

When an agent has worked out a job that takes several calls, it can save its program as a new tool, and the next session does the job in one call:

```sh
~/mcp/forge latest_releases program.ts     # or the `forge` tool in code mode
~/mcp/forged/latest_releases --org tokio-rs
```

The program starts with a short header:

```ts
// @description Latest release of each of an org's 3 most-starred repos.
// @param org string required The GitHub org
const top = await tools.github.search_repositories({ query: `org:${args.org}`, sort: "stars", perPage: 3 });
// ...
console.log(JSON.stringify(result));
```

A forged tool works like any other: `--help`, a line in the README, `tools.forged.<name>()` in a program. It runs in the sandbox and may only call the tools its code names. It survives `dogu sync`.

Measured over a job repeated for five GitHub orgs, on Sonnet: once the tool exists, each job costs 28 to 31% less, is faster, and in the folder takes 2 turns instead of 3. Forging pays for itself after 3 more jobs in code mode and 9 in the folder (the session after forging refreshes the prompt cache once). Agents used their forged tool in every later session, 24 of 24.

## Logging in to remote servers

Many hosted servers want an OAuth login. If `dogu sync` says a server needs one:

```
dogu login
dogu sync
```

- `dogu login` opens your browser for each server that needs it. `dogu login <server>` does one. `dogu logout <server>` forgets it.
- Logins are saved in `~/.config/dogu/auth.json`, which only you can read. Tokens are refreshed when they expire.
- A tool never opens a browser by itself. If a login has run out, the tool fails and says `Run: dogu login <server>`.
- On a machine with no browser, set `DOGU_NO_BROWSER=1` and open the printed link somewhere else. The link comes back to `127.0.0.1:38573` on the machine that runs dogu, so forward that port: `ssh -L 38573:127.0.0.1:38573 ...`

<details>
<summary>How dogu gets a client ID</summary>

In this order:

1. From your config, if you registered an app with the service yourself: `"oauth": { "client_id": "...", "client_secret": "..." }` next to the `"url"`.
2. From a Client ID Metadata Document, if you host one. Put `client-metadata.json` from this repo on an HTTPS URL you own, fix the URLs inside it, and set `DOGU_CLIENT_ID_URL` to that URL. The current MCP spec prefers this way.
3. By registering itself with the service (Dynamic Client Registration). This needs no setup and is the default, but the spec has deprecated it.

</details>

## Keep an agent to read-only tools

Servers can say what each tool may change. The README shows it next to each tool, `[read-only]`, `[changes things]` or `[may delete or overwrite]`, and so does `--help`. dogu never guesses: a tool the server says nothing about gets no label.

```
dogu sync --read-only     # keep only the tools the servers mark read-only
dogu sync --all-tools     # bring the rest back
```

`--read-only` is remembered for later syncs. A tool it leaves out has no command, is not in code mode, and `dogu call` refuses it. A server that marks nothing loses all its tools: with GitHub's official server, Context7 and DeepWiki, it keeps 28 of GitHub's 46 tools, both of Context7's, and none of DeepWiki's.

## What the sandbox protects, and what it does not

A program run by `dogu run` or `run_code` runs in djs, which has no file, network, environment or process APIs at all: its only way out is calling your tools. Each run has a memory limit and a deadline it cannot catch. `tests/escape.ts` tries 17 ways out. All 17 are blocked, in djs and in Deno:

| Blocked | |
|---|---|
| Reading or writing files | `Deno.readTextFile`, `node:fs`, `fetch("file://...")` |
| The network | `fetch`, `WebSocket`, importing from a URL, `npm:` and `jsr:` imports |
| Your environment | `Deno.env`, `process.env`, the hostname |
| Other programs | `Deno.Command`, `node:child_process`, FFI |
| Importing local files | static and dynamic `import` of any file on disk |

The limits of that protection:

- **It does not make your tools safe.** If a tool can delete files or send email, a program can call it. Use `--allow` to give a program only the tools the job needs.
- **It only matters if the agent has no other way out.** An agent with a shell can skip the sandbox. For the full benefit, give the agent `dogu serve` and no shell.
- **It is a JavaScript engine, not a virtual machine.** djs is young; a bug in it would be a bug here. Deno, with `DOGU_SANDBOX=deno`, is the older, more tested choice.

How the two engines compare on the same dogu binary (median of 20 runs, `bench/sandbox_speed.py`):

| Program | Deno | djs |
|---|---|---|
| `console.log(1)` | 14.7 ms | **2.3 ms** |
| 10 tool calls to a local server | 34.2 ms | **21.0 ms** |
| JSON: build, parse and filter 20,000 items (1.3 MB) | **23.2 ms** | 26.5 ms |
| A 2,000,000-step loop | **16.9 ms** | 31.5 ms |

djs starts fast and has no JIT, so programs that call tools and filter their results are faster, and long pure-computation loops are slower.

Every tool call is recorded in `~/mcp/.dogu/log.jsonl`: the time, the tool, whether it worked, how long it took. Parameters and results are not recorded.

## Protocol support

MCP changed shape in its 2026-07-28 release: the `initialize` handshake and sessions are gone, and every request says which version it speaks. dogu speaks both the new protocol and the older one. It tries the new one first, falls back, and remembers the answer for each server.

Transports: stdio and Streamable HTTP.

## Tested

- **The official MCP conformance suite**, `@modelcontextprotocol/conformance`. 35 client scenarios pass: 8 for the protocol and the 27 core OAuth ones (discovery, PKCE, scopes, step-up, issuer checks, token endpoint auth, registration). Run them with `sh tests/conformance.sh`.
- **The sandbox escape test**, `tests/escape.ts`.
- **Real servers**: the official filesystem, memory and everything servers, a server built with the v2 TypeScript SDK, the SDK's OAuth demo server, and these hosted ones: GitHub's official server (with a token), Linear (with `dogu login`, including token refresh), Context7 and DeepWiki.
- **A real agent**: Claude Code, told only "Your tools are in ~/mcp. Read ~/mcp/README.md first.", used the folder for a three-step job with no errors, and did the same through `dogu serve` with no shell.
- **A benchmark** (`bench/`, written with [Effect](https://effect.website)): Claude Code, Codex and pi, each with the servers loaded directly and through dogu, on tasks written before any run, with every answer checked. Over 1,700 runs. Results are in `bench/results/`.

## Status

This is version 0.7.1 and it is new. Here is what has not been done yet:

- It has been built and run on **Linux and macOS**. Windows does not work: the tools are `sh` scripts.
- Of the **hosted servers**, GitHub, Linear, Context7 and DeepWiki have been tried. Notion and others have not. Real services have quirks. If one breaks, please open an issue with the output of `DOGU_DEBUG=1 dogu sync`.
- **Tools that ask the user a question mid-call** (elicitation, sampling) are not supported. The call fails with a clear message.
- **Resources and prompts** are not exposed. Tools only.
- **When a local server dies at start**, dogu shows what it said. (Inside Codex's sandbox, npx servers work without any setting since 0.6.0.)
- **The folder's background dogu is Unix only.** On a system without it, each command starts its server, runs the tool and stops it.
- **Types are stripped, not checked**, before a program runs. A wrong parameter shows up as an error from the tool.
- **The old SSE transport** from 2024 is not supported.

## Questions

**Is this a real filesystem mount?**
No. It is a normal folder of small scripts that `dogu sync` writes. A real mount needs FUSE, which is painful to install on macOS and different again on Windows. A mount also makes reading a file run a tool, so `grep -r` or a file indexer would call your tools by accident.

**Which agents does it work with?**
The folder works with any agent that can run shell commands. `dogu serve` works with any app that can connect to an MCP server.

**Is this "code mode"?**
Yes, in two forms. In the sandbox and in `dogu serve`, the agent writes TypeScript that calls tools. In the folder, it writes shell: each tool is a command, and pipes carry results from one tool to the next. Either way, results flow between tools without passing through the model, and it all runs on your machine with nothing to install but dogu.

**Do I have to change my MCP servers?**
No. dogu is a client. It reads the config you already have.

**Where does the name come from?**
"Dogu" is Japanese for "tools". It also has a dog in it, and the dog fetches them.

## Configuration

The usual `mcp.json`:

```json
{
  "mcpServers": {
    "files":  { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/Users/me/Documents"] },
    "remote": { "url": "https://example.com/mcp", "headers": { "Authorization": "Bearer ${MY_TOKEN}" } }
  }
}
```

- `${NAME}` is replaced with that environment variable, so secrets can stay out of the file.
- A local server runs in the folder the config file is in, unless it has a `"cwd"`.
- A remote server with no `Authorization` header uses the login flow when the server asks for one.

| Variable | What it does |
|---|---|
| `DOGU_CONFIG` | Path to the config file |
| `DOGU_TIMEOUT` | Seconds to wait: 300 for one tool, 120 for a whole program |
| `DOGU_MEMORY_MB` | Memory limit for a program. Default 256 |
| `DOGU_MAX_OUTPUT` | Most bytes a program may print. Default 1000000 |
| `DOGU_CALLBACK_PORT` | Port the browser comes back to after a login. Default 38573 |
| `DOGU_CLIENT_ID_URL` | URL of your hosted client metadata document |
| `DOGU_NO_BROWSER` | Print the login link and do not open a browser |
| `DOGU_DEBUG` | Show what local servers print, and more detail on failures |
| `DOGU_WARM` | `0`: no background dogu for the folder; every command connects on its own |
| `DOGU_WARM_IDLE` | Seconds the folder's background dogu waits for a call before it leaves. Default 600 |
| `DOGU_CACHE_SECS` | Seconds a read-only tool's answer is kept, so the same call again is answered at once. Default 30; `0` turns it off |

## How it is built

About 4,200 lines of Rust and JavaScript, with six dependencies and no async runtime.

| File | Job |
|---|---|
| `src/config.rs` | Read `mcp.json` |
| `src/client.rs` | Talk MCP over stdio or HTTP, in both protocol eras |
| `src/auth.rs` | OAuth login: discovery, the browser round trip, tokens, refresh |
| `src/sync.rs` | Write the folder: the commands, the READMEs, the TypeScript types |
| `src/call.rs` | Run one tool from the command line |
| `src/run.rs` | Start the sandbox and answer the program's tool calls |
| `src/sandbox_djs.rs` | Run programs in djs, the built-in JavaScript engine |
| `src/runner.js` | The only trusted code inside the sandbox |
| `src/serve.rs` | Be an MCP server with `search` and `run_code` |

## Roadmap

1. Keep local servers running between commands, so `npx` servers are fast in the folder too.
2. Resources as plain read-only files in each app's folder.
3. A Homebrew formula.
4. Windows.


Issues and pull requests are welcome. The most useful report right now is "I ran it against server X and this happened".

## License

MIT. See [LICENSE](LICENSE).
