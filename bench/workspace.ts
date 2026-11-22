// What every benchmark run needs: a temp folder with the configs, a synced dogu folder, an optional
// seeded memory server, and the Claude Code arguments for each way of giving an agent the tools.

import { Console, Duration, Effect } from "npm:effect@4.1.5";
import { CommandFailed, runClaude, runCommand } from "./claude.ts";
import type { Ctx } from "./tasks.ts";

export const RUN_TIMEOUT = Duration.minutes(5);
export const ANSWER_RULE = " Use only the tools you have been given. Answer in under 6 lines.";
export const ALL_SETUPS = ["direct", "direct-search", "folder", "folder-read", "codemode"] as const;
export type Setup = (typeof ALL_SETUPS)[number];

const HERE = new URL(".", import.meta.url).pathname;

export interface Workspace {
  readonly dir: string;
  readonly root: string; // the dogu folder
  readonly directConfig: string; // Claude Code config with every server, token included
  readonly serveConfig: string; // Claude Code config with only `dogu serve`
  readonly token: string;
  readonly mcpServers: ReadonlyArray<string>; // `mcp__<name>` for each server, for --allowedTools
  readonly seeded: Ctx["seeded"];
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
  return { text: lines.join("\n") + "\n", seeded: { word, entity: `Project ${names[pick]}` } };
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
      yield* writePrivate(
        `${dir}/serve.json`,
        JSON.stringify({ mcpServers: { tools: { command: "dogu", args: ["serve", "--root", root], env: { GITHUB_TOKEN: token } } } }),
      );
      yield* runCommand("dogu", ["sync", "--config", `${dir}/servers.json`, "--out", root], { env: { GITHUB_TOKEN: token } });
      const mcpServers = Object.keys(direct.mcpServers).map((n) => `mcp__${n}`);
      return { dir, root, directConfig: `${dir}/direct.json`, serveConfig: `${dir}/serve.json`, token, mcpServers, seeded: memory.seeded };
    }),
    (ws) => Effect.tryPromise(() => Deno.remove(ws.dir, { recursive: true })).pipe(Effect.orElseSucceed(() => undefined)),
  );

export const workDir = (ws: Workspace, setup: Setup) => `${ws.dir}/work-${setup}`;

/**
 * The Claude Code arguments for one run. `runId` starts the prompt so no two conversations are alike:
 * the standing part (system prompt, tool definitions, README) stays cached between runs, as it does
 * between a person's sessions, but no run can replay an earlier conversation from the cache.
 * The folder's README is read fresh each run, as a CLAUDE.md import would, so newly forged tools show up.
 */
export const argsFor = (setup: Setup, prompt: string, ws: Workspace, runId: string, model: string) =>
  Effect.gen(function* () {
    // "folder-read" gets the one line every agent can be given; "folder" has the README in its instructions.
    const told = setup === "folder-read" ? `Your tools are in ${ws.root}. Read ${ws.root}/README.md first. ` : "";
    const common = ["-p", `Run ${runId}. ${told}${prompt}`, "--model", model, "--setting-sources", "project", "--strict-mcp-config"];
    const bash = ["--mcp-config", '{"mcpServers":{}}', "--tools", "Bash", "Read", "--allowedTools", "Bash", "Read"];
    const standing = setup === "folder"
      ? `Your tools are in ${ws.root}. This is its README:\n\n${yield* Effect.promise(() => Deno.readTextFile(`${ws.root}/README.md`))}`
      : "";
    const tools: Record<Setup, ReadonlyArray<string>> = {
      direct: ["--mcp-config", ws.directConfig, "--tools", "", "--allowedTools", ...ws.mcpServers],
      // Claude Code's own answer to the same problem: tool definitions deferred, found with ToolSearch.
      "direct-search": ["--mcp-config", ws.directConfig, "--tools", "ToolSearch", "--allowedTools", "ToolSearch", ...ws.mcpServers],
      folder: ["--append-system-prompt", standing, ...bash],
      "folder-read": bash,
      codemode: [
        "--mcp-config", ws.serveConfig, "--tools", "",
        "--allowedTools", "mcp__tools__search", "mcp__tools__run_code", "mcp__tools__forge",
      ],
    };
    return [...common, ...tools[setup], "--output-format", "stream-json", "--verbose"] as ReadonlyArray<string>;
  });

export const envFor = (setup: Setup, ws: Workspace): Record<string, string> =>
  setup === "direct-search" ? { GITHUB_TOKEN: ws.token, ENABLE_TOOL_SEARCH: "true" } : { GITHUB_TOKEN: ws.token };

/** One short session before measuring, so a setup starts with its standing part cached. */
export const warmUp = (ws: Workspace, setup: Setup, model: string) =>
  Effect.gen(function* () {
    yield* Effect.promise(() => Deno.mkdir(workDir(ws, setup), { recursive: true }));
    yield* runClaude(yield* argsFor(setup, "Reply with just: ok", ws, "warmup", model), {
      cwd: workDir(ws, setup),
      env: envFor(setup, ws),
      timeout: RUN_TIMEOUT,
    });
    yield* Console.log(`  warmed  ${setup}`);
  });
