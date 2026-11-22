// What every benchmark run needs: a temp folder with the configs, a synced dogu folder, an optional
// seeded memory server, and the Claude Code arguments for each way of giving an agent the tools.

import { Console, Duration, Effect } from "npm:effect@4.1.5";
import { type Agent, CommandFailed, runAgent, runCommand } from "./claude.ts";
import type { Ctx } from "./tasks.ts";

export const RUN_TIMEOUT = Duration.minutes(5);
export const ANSWER_RULE = " Use only the tools you have been given. Answer in under 6 lines.";
export const ALL_SETUPS = [
  "direct", "direct-search", "folder", "folder-read", "folder-b", "codemode", "codemode-b", "utcp",
  "codex-direct", "codex-folder", "codex-codemode", "codex-dogu",
  "pi-direct", "pi-codemode", "pi-folder", "pi-dogu",
] as const;
/** pi runs the same model as Claude Code's `sonnet`, with thinking off as in Claude Code's default. */
const PI_MODEL = Deno.env.get("PI_MODEL") ?? "anthropic/claude-sonnet-5-5";
/** pi's reasoning effort. gpt-6.2 models do not allow "off"; their lowest is "low". */
const PI_THINKING = Deno.env.get("PI_THINKING") ?? "off";
/**
 * An A/B test of two dogu builds in one run, so both meet the same API speed: `codemode` serves with the
 * dogu on your PATH, `codemode-b` with the binary at BENCH_DOGU_B.
 */
const DOGU_B = Deno.env.get("BENCH_DOGU_B");

/** UTCP code-mode, the most-starred standalone code mode for MCP, pinned so a run can be repeated. */
const UTCP_PACKAGE = "@utcp/code-mode-mcp@1.2.1";

/** The Codex binary; set CODEX_BIN when it is not on your PATH. CODEX_MODEL picks a model, else Codex's default. */
const CODEX_BIN = Deno.env.get("CODEX_BIN") ?? "codex";
const CODEX_MODEL = Deno.env.get("CODEX_MODEL");
export type Setup = (typeof ALL_SETUPS)[number];

const HERE = new URL(".", import.meta.url).pathname;

export interface Workspace {
  readonly dir: string;
  readonly root: string; // the dogu folder
  readonly rootB: string; // the same folder synced by the BENCH_DOGU_B build, for folder-b
  readonly directConfig: string; // Claude Code config with every server, token included
  readonly serveConfig: string; // Claude Code config with only `dogu serve`
  readonly serveConfigB: string; // the same with the BENCH_DOGU_B build
  readonly utcpConfig: string; // Claude Code config with only UTCP code-mode, given the same servers
  readonly token: string;
  readonly mcpServers: ReadonlyArray<string>; // `mcp__<name>` for each server, for --allowedTools
  readonly seeded: Ctx["seeded"];
  readonly memoryFile: string | null; // the memory server's file, when it runs
  readonly codexHome: string; // CODEX_HOME: your Codex login linked in, nothing else of yours
  readonly hosted: Record<string, unknown>; // the hosted servers, with ${GITHUB_TOKEN} left for the agent to fill in
}

const writePrivate = (path: string, text: string) =>
  Effect.tryPromise({
    try: () => Deno.writeTextFile(path, text, { mode: 0o600 }),
    catch: (e) => new CommandFailed({ command: `write ${path}`, detail: String(e) }),
  });

/** A made-up word no model has seen, so an answer that has it came from the tools. */
const randomWord = () => {
  const syllables = ["ka", "zo", "ri", "ve", "mu", "tal", "dro", "pen", "qui", "nax", "bel", "sor"];
  return Array.from({ length: 4 }, () => syllables[crypto.getRandomValues(new Uint8Array(1))[0] % syllables.length]).join("");
};

/** A memory-server graph of 12 projects, one of which has an observation with a random word. */
const seedMemory = () => {
  const names = ["Alder", "Birch", "Cedar", "Dogwood", "Elm", "Fir", "Ginkgo", "Hazel", "Ivy", "Juniper", "Larch", "Maple"];
  const word = randomWord();
  const pick = crypto.getRandomValues(new Uint8Array(1))[0] % names.length;
  const lines = names.map((n, i) =>
    JSON.stringify({
      type: "entity",
      name: `Project ${n}`,
      entityType: "project",
      observations: [`Started in ${2015 + i}`, i === pick ? `Its build uses the ${word} toolchain` : `Owned by team ${randomWord()}`],
    })
  );
  // A cut-off year for heldout2's memory-years task: the projects started 2015 to 2026, one a year.
  const year = 2016 + (crypto.getRandomValues(new Uint8Array(1))[0] % 10);
  return {
    text: lines.join("\n") + "\n",
    seeded: { word, entity: `Project ${names[pick]}`, year, startedThen: `Project ${names[year - 2015]}`, entityYear: 2015 + pick },
  };
};

/**
 * A temp folder with the configs and a synced dogu folder, plus the local memory server when asked.
 * Removed when the scope ends, even on failure. The GitHub token is written only here, readable by you only.
 */
export const makeWorkspace = (options: { readonly memory: boolean }) =>
  Effect.acquireRelease(
    Effect.gen(function* () {
      const token = (yield* runCommand("gh", ["auth", "token"])).trim();
      const dir = yield* Effect.promise(() => Deno.makeTempDir({ prefix: "dogu-bench-" }));
      const memory = seedMemory();
      const hosted = JSON.parse(yield* Effect.promise(() => Deno.readTextFile(`${HERE}servers.json`))).mcpServers;
      const memoryServer = {
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-memory"],
        env: { MEMORY_FILE_PATH: `${dir}/memory.jsonl` },
      };
      const local = options.memory ? { memory: memoryServer } : {};
      if (options.memory) yield* writePrivate(`${dir}/memory.jsonl`, memory.text);
      // dogu's config keeps ${GITHUB_TOKEN} for dogu to fill in; Claude Code's gets the token itself.
      const withToken = JSON.parse(JSON.stringify(hosted).replaceAll("${GITHUB_TOKEN}", token));
      const direct = {
        mcpServers: {
          ...Object.fromEntries(Object.entries(withToken).map(([name, s]) => [name, { type: "http", ...(s as object) }])),
          ...(options.memory ? { memory: { type: "stdio", ...memoryServer } } : {}),
        },
      };
      const root = `${dir}/mcp`;
      yield* writePrivate(`${dir}/servers.json`, JSON.stringify({ mcpServers: { ...hosted, ...local } }));
      yield* writePrivate(`${dir}/direct.json`, JSON.stringify(direct));
      const serve = (command: string) =>
        JSON.stringify({ mcpServers: { tools: { command, args: ["serve", "--root", root], env: { GITHUB_TOKEN: token } } } });
      yield* writePrivate(`${dir}/serve.json`, serve("dogu"));
      yield* writePrivate(`${dir}/serve-b.json`, serve(DOGU_B ?? "dogu"));
      // UTCP code-mode gets the same servers and token, in its own config format.
      const utcpServers = Object.fromEntries(
        Object.entries(direct.mcpServers).map(([name, s]) => {
          const { type, ...rest } = s as { type: string };
          return [name, { transport: type === "http" ? "http" : "stdio", ...rest }];
        }),
      );
      yield* writePrivate(
        `${dir}/utcp-config.json`,
        JSON.stringify({
          tool_repository: { tool_repository_type: "in_memory" },
          tool_search_strategy: { tool_search_strategy_type: "tag_and_description_word_match" },
          manual_call_templates: [{ name: "servers", call_template_type: "mcp", config: { mcpServers: utcpServers } }],
        }),
      );
      yield* writePrivate(
        `${dir}/utcp.json`,
        JSON.stringify({
          mcpServers: { utcp: { command: "npx", args: ["-y", UTCP_PACKAGE], env: { UTCP_CONFIG_FILE: `${dir}/utcp-config.json` } } },
        }),
      );
      yield* runCommand("dogu", ["sync", "--config", `${dir}/servers.json`, "--out", root], { env: { GITHUB_TOKEN: token } });
      // The folder's commands run the dogu that synced it, so folder-b is synced by the build under test.
      const rootB = `${dir}/mcp-b`;
      yield* runCommand(DOGU_B ?? "dogu", ["sync", "--config", `${dir}/servers.json`, "--out", rootB], { env: { GITHUB_TOKEN: token } });
      const mcpServers = Object.keys(direct.mcpServers).map((n) => `mcp__${n}`);
      // Codex gets its own home with only your login in it, linked, not copied.
      const codexHome = `${dir}/codex-home`;
      yield* Effect.promise(() => Deno.mkdir(codexHome, { recursive: true }));
      const login = `${Deno.env.get("HOME")}/.codex/auth.json`;
      yield* Effect.promise(() => Deno.symlink(login, `${codexHome}/auth.json`).catch(() => undefined));
      return {
        dir, root, rootB, directConfig: `${dir}/direct.json`, serveConfig: `${dir}/serve.json`, serveConfigB: `${dir}/serve-b.json`,
        utcpConfig: `${dir}/utcp.json`,
        token, mcpServers, seeded: memory.seeded,
        memoryFile: options.memory ? `${dir}/memory.jsonl` : null, codexHome, hosted,
      };
    }),
    (ws) => Effect.tryPromise(() => Deno.remove(ws.dir, { recursive: true })).pipe(Effect.orElseSucceed(() => undefined)),
  );

export const workDir = (ws: Workspace, setup: Setup) => `${ws.dir}/work-${setup}`;

/** How to run one session: which agent, which program, which arguments. */
export interface Call {
  readonly agent: Agent;
  readonly bin: string;
  readonly args: ReadonlyArray<string>;
}

const readme = (ws: Workspace, root = ws.root) => Effect.promise(() => Deno.readTextFile(`${root}/README.md`));

/** Codex's settings for one run: its sandbox stays on with network allowed, as dogu's tools need it. */
const codexArgs = (prompt: string, cwd: string, extra: ReadonlyArray<string>) => [
  "exec", "--json", "--ephemeral", "--ignore-user-config", "--skip-git-repo-check",
  "-s", "workspace-write", "-c", "sandbox_workspace_write.network_access=true",
  // Codex hides variables named like *TOKEN* from commands by default; dogu's tools need GITHUB_TOKEN.
  "-c", "shell_environment_policy.ignore_default_excludes=true",
  "-c", 'approval_policy="never"',
  // npx writes its cache in ~/.npm, which Codex's sandbox blocks, so local servers started through dogu
  // die. dogu's README tells Codex users to allow it; BENCH_CODEX_NPM=0 measures without.
  ...(Deno.env.get("BENCH_CODEX_NPM") === "0"
    ? []
    : ["-c", `sandbox_workspace_write.writable_roots=[${JSON.stringify(`${Deno.env.get("HOME")}/.npm`)}]`]),
  ...(CODEX_MODEL ? ["-m", CODEX_MODEL] : []),
  // CODEX_FAST=1: Codex's fast mode, OpenAI's priority processing. It uses more of your plan.
  ...(Deno.env.get("CODEX_FAST") === "1" ? ["-c", 'service_tier="fast"'] : []),
  "-C", cwd, ...extra, prompt,
];

/** pi's own config folder for a setup: your pi login linked in, our servers, nothing else of yours. */
export const piAgentDir = (ws: Workspace, setup: Setup) => `${ws.dir}/pi-agent-${setup}`;

/**
 * pi setups. pi-direct declares every MCP tool; pi-codemode is pi's own code mode, where MCP tools are
 * reached from scripts; pi-folder is the dogu folder, with the README as appended system instructions.
 */
const piArgs = (setup: Setup, prompt: string, ws: Workspace) =>
  Effect.gen(function* () {
    const agentDir = piAgentDir(ws, setup);
    yield* Effect.promise(() => Deno.mkdir(agentDir, { recursive: true }));
    const login = `${Deno.env.get("HOME")}/.pi/agent/auth.json`;
    yield* Effect.promise(() => Deno.symlink(login, `${agentDir}/auth.json`).catch(() => undefined));
    const exposure = setup === "pi-direct" ? "direct" : "codemode";
    const servers = {
      ...Object.fromEntries(Object.entries(ws.hosted).map(([n, s]) => [n, { ...(s as object), exposure }])),
      ...(ws.memoryFile
        ? { memory: { command: "npx", args: ["-y", "@modelcontextprotocol/server-memory"], env: { MEMORY_FILE_PATH: ws.memoryFile }, exposure } }
        : {}),
    };
    if (setup === "pi-dogu") {
      // dogu's code mode in pi: one MCP server, `dogu serve`, whose three tools pi declares directly.
      const dogu = { command: DOGU_B ?? "dogu", args: ["serve", "--root", ws.root], env: { GITHUB_TOKEN: ws.token }, exposure: "direct" };
      yield* writePrivate(`${agentDir}/mcp.json`, JSON.stringify({ mcpServers: { tools: dogu } }));
    } else if (setup === "pi-folder") {
      yield* Effect.promise(() => Deno.writeTextFile(`${agentDir}/APPEND_SYSTEM.md`, `Your tools are in ${ws.root}. This is its README:\n\n`));
      const text = yield* readme(ws);
      yield* Effect.promise(() => Deno.writeTextFile(`${agentDir}/APPEND_SYSTEM.md`, text, { append: true }));
    } else {
      yield* writePrivate(`${agentDir}/mcp.json`, JSON.stringify({ mcpServers: servers }));
    }
    const tools = {
      "pi-direct": ["--tools", "mcp__*"],
      "pi-codemode": ["--tools", "codemode"],
      "pi-folder": ["--tools", "bash,read", "--no-mcp"],
      "pi-dogu": ["--tools", "mcp__*"],
    }[setup as "pi-direct" | "pi-codemode" | "pi-folder" | "pi-dogu"];
    return ["--mode", "json", "--no-session", "--no-skills", "--no-context-files", "--offline", "--model", PI_MODEL, "--thinking", PI_THINKING, ...tools, prompt];
  });

/** Our servers as Codex config overrides. */
/**
 * BENCH_CODEX_APPROVE=1: trust every MCP server's tools, as a person does after allowing a server.
 * Without it, Codex asks before calling a tool not marked read-only, and in a headless run nobody can
 * answer, so the call is refused. Applied to every Codex setup alike.
 */
const CODEX_APPROVE = Deno.env.get("BENCH_CODEX_APPROVE") === "1";
const approve = (names: ReadonlyArray<string>) =>
  CODEX_APPROVE ? names.flatMap((n) => ["-c", `mcp_servers.${n}.default_tools_approval_mode="approve"`]) : [];

const codexServers = (ws: Workspace) => [
  ...approve(["github", "context7", "deepwiki", ...(ws.memoryFile ? ["memory"] : [])]),
  "-c", 'mcp_servers.github.url="https://api.githubcopilot.com/mcp/"',
  "-c", 'mcp_servers.github.bearer_token_env_var="GITHUB_TOKEN"',
  "-c", 'mcp_servers.context7.url="https://mcp.context7.com/mcp"',
  "-c", 'mcp_servers.deepwiki.url="https://mcp.deepwiki.com/mcp"',
  ...(ws.memoryFile
    ? [
      "-c", 'mcp_servers.memory.command="npx"',
      "-c", 'mcp_servers.memory.args=["-y","@modelcontextprotocol/server-memory"]',
      "-c", `mcp_servers.memory.env={MEMORY_FILE_PATH=${JSON.stringify(ws.memoryFile)}}`,
    ]
    : []),
];

/**
 * The arguments for one run. `runId` starts the prompt so no two conversations are alike: the
 * standing part (system prompt, tool definitions, README) stays cached between runs, as it does
 * between a person's sessions, but no run can replay an earlier conversation from the cache.
 * The folder's README is read fresh each run, as a CLAUDE.md import would, so newly forged tools show up.
 */
export const argsFor = (setup: Setup, prompt: string, ws: Workspace, runId: string, model: string) =>
  Effect.gen(function* () {
    const cwd = workDir(ws, setup);
    // Codex read a bare "Run <id>." as something to look up, so for Codex the ID is a labelled note at the end.
    // It still makes every conversation unique, which is all it is for.
    const codexPrompt = `${prompt} (Run ID ${runId}, for logging only; ignore it.)`;
    if (setup.startsWith("pi-")) {
      return { agent: "pi", bin: "pi", args: yield* piArgs(setup, `Run ${runId}. ${prompt}`, ws) } as Call;
    }
    if (setup === "codex-direct") {
      return { agent: "codex", bin: CODEX_BIN, args: codexArgs(codexPrompt, cwd, codexServers(ws)) } as Call;
    }
    if (setup === "codex-codemode") {
      // Codex's own code mode (a feature still in development in Codex 0.171): the agent reaches the
      // same MCP servers only through code.
      const flags = ["-c", "features.code_mode=true", "-c", "features.code_mode_only=true"];
      return { agent: "codex", bin: CODEX_BIN, args: codexArgs(codexPrompt, cwd, [...codexServers(ws), ...flags]) } as Call;
    }
    if (setup === "codex-dogu") {
      // dogu's code mode in Codex: `dogu serve` as the only MCP server. The token is passed by name.
      const dogu = [
        "-c", `mcp_servers.tools.command=${JSON.stringify(DOGU_B ?? "dogu")}`,
        "-c", `mcp_servers.tools.args=${JSON.stringify(["serve", "--root", ws.root])}`,
        "-c", 'mcp_servers.tools.env_vars=["GITHUB_TOKEN"]',
        ...approve(["tools"]),
      ];
      return { agent: "codex", bin: CODEX_BIN, args: codexArgs(codexPrompt, cwd, dogu) } as Call;
    }
    if (setup === "codex-folder") {
      // A Codex user puts standing instructions in AGENTS.md; Codex reads it from the working folder.
      const text = `Your tools are in ${ws.root}. This is its README:\n\n${yield* readme(ws)}`;
      yield* Effect.promise(() => Deno.writeTextFile(`${cwd}/AGENTS.md`, text));
      return { agent: "codex", bin: CODEX_BIN, args: codexArgs(codexPrompt, cwd, []) } as Call;
    }
    // "folder-read" gets the one line every agent can be given; "folder" has the README in its instructions.
    const told = setup === "folder-read" ? `Your tools are in ${ws.root}. Read ${ws.root}/README.md first. ` : "";
    const common = ["-p", `Run ${runId}. ${told}${prompt}`, "--model", model, "--setting-sources", "project", "--strict-mcp-config"];
    const bash = ["--mcp-config", '{"mcpServers":{}}', "--tools", "Bash", "Read", "--allowedTools", "Bash", "Read"];
    const standing = setup === "folder" ? `Your tools are in ${ws.root}. This is its README:\n\n${yield* readme(ws)}` : "";
    const standingB = setup === "folder-b" ? `Your tools are in ${ws.rootB}. This is its README:\n\n${yield* readme(ws, ws.rootB)}` : "";
    const codemode = (config: string) => [
      "--mcp-config", config, "--tools", "",
      "--allowedTools", "mcp__tools__search", "mcp__tools__run_code", "mcp__tools__forge",
    ];
    const tools: Record<"direct" | "direct-search" | "folder" | "folder-read" | "folder-b" | "codemode" | "codemode-b" | "utcp", ReadonlyArray<string>> = {
      direct: ["--mcp-config", ws.directConfig, "--tools", "", "--allowedTools", ...ws.mcpServers],
      // Claude Code's own answer to the same problem: tool definitions deferred, found with ToolSearch.
      "direct-search": ["--mcp-config", ws.directConfig, "--tools", "ToolSearch", "--allowedTools", "ToolSearch", ...ws.mcpServers],
      folder: ["--append-system-prompt", standing, ...bash],
      "folder-read": bash,
      "folder-b": ["--append-system-prompt", standingB, ...bash],
      codemode: codemode(ws.serveConfig),
      "codemode-b": codemode(ws.serveConfigB),
      utcp: ["--mcp-config", ws.utcpConfig, "--tools", "", "--allowedTools", "mcp__utcp"],
    };
    const claudeSetup = setup as keyof typeof tools;
    return { agent: "claude", bin: "claude", args: [...common, ...tools[claudeSetup], "--output-format", "stream-json", "--verbose"] } as Call;
  });

export const envFor = (setup: Setup, ws: Workspace): Record<string, string> => ({
  GITHUB_TOKEN: ws.token,
  ...(setup === "direct-search" ? { ENABLE_TOOL_SEARCH: "true" } : {}),
  ...(setup.startsWith("codex") ? { CODEX_HOME: ws.codexHome } : {}),
  ...(setup.startsWith("pi-") ? { PI_CODING_AGENT_DIR: piAgentDir(ws, setup) } : {}),
});

/** One short session before measuring, so a setup starts with its standing part cached. */
export const warmUp = (ws: Workspace, setup: Setup, model: string) =>
  Effect.gen(function* () {
    yield* Effect.promise(() => Deno.mkdir(workDir(ws, setup), { recursive: true }));
    const call = yield* argsFor(setup, "Reply with just: ok", ws, "warmup", model);
    yield* runAgent(call.agent, call.bin, call.args, {
      cwd: workDir(ws, setup),
      env: envFor(setup, ws),
      timeout: RUN_TIMEOUT,
    });
    yield* Console.log(`  warmed  ${setup}`);
  });
