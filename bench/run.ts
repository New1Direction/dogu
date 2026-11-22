// dogu benchmark: the same tasks, done by Claude Code with the tools given five ways.
//
//   deno run --allow-run --allow-env --allow-read --allow-write --allow-net bench/run.ts
//
// Needs: claude (logged in), gh (logged in), dogu, Node (for the local memory server) and Deno.
// Settings: BENCH_REPS (3), BENCH_CONCURRENCY (3), BENCH_MODEL (sonnet), BENCH_SUITE (all, tuning or heldout),
// BENCH_TASKS and BENCH_SETUPS (all, comma lists), BENCH_OUT (a name for the results files instead of the date),
// BENCH_DRY=1 (fetch and print the right answers, run no agents).

import { Console, Data, Duration, Effect, Result } from "npm:effect@4.1.5";
import { CommandFailed, runClaude, runCommand } from "./claude.ts";
import { headline, type RunRecord, summaryTable, taskTable } from "./report.ts";
import { type Ctx, type Facts, promptOf, type Task, TASKS } from "./tasks.ts";

class BadTruth extends Data.TaggedError("BadTruth")<{ readonly detail: string }> {}

const env = (name: string, fallback: string) => Deno.env.get(name) ?? fallback;
const list = (name: string) => env(name, "").split(",").filter((s) => s !== "");
const REPS = Number(env("BENCH_REPS", "3"));
const CONCURRENCY = Number(env("BENCH_CONCURRENCY", "3"));
const MODEL = env("BENCH_MODEL", "sonnet");
const SUITE = env("BENCH_SUITE", "all");
const RUN_TIMEOUT = Duration.minutes(5);
const HERE = new URL(".", import.meta.url).pathname;

const ALL_SETUPS = ["direct", "direct-search", "folder", "folder-read", "codemode"] as const;
type Setup = (typeof ALL_SETUPS)[number];
const SETUPS: ReadonlyArray<Setup> = ALL_SETUPS.filter((s) => list("BENCH_SETUPS").length === 0 || list("BENCH_SETUPS").includes(s));
const TASKS_RUN = TASKS.filter((t) =>
  (SUITE === "all" || t.suite === SUITE) && (list("BENCH_TASKS").length === 0 || list("BENCH_TASKS").includes(t.id))
);

const ANSWER_RULE = " Use only the tools you have been given. Answer in under 6 lines.";
const MCP_SERVERS = ["mcp__github", "mcp__context7", "mcp__deepwiki", "mcp__memory"];

interface Workspace {
  readonly dir: string;
  readonly root: string; // the dogu folder
  readonly directConfig: string; // Claude Code config with every server, token included
  readonly serveConfig: string; // Claude Code config with only `dogu serve`
  readonly token: string;
  /** The folder's README as standing instructions: what `@~/mcp/README.md` in CLAUDE.md, or pasting it into AGENTS.md, gives. */
  readonly standing: string;
  readonly seeded: Ctx["seeded"];
}

// --- Setup and cleanup ---

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

/**
 * A memory-server graph of 12 projects, one of which has an observation with a random word.
 * The held-out task `memory-find` asks which one.
 */
const seedMemory = (path: string) => {
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

/** A temp folder with the configs, a seeded memory graph and a synced dogu folder. Removed at the end, even on failure. */
const workspace = Effect.acquireRelease(
  Effect.gen(function* () {
    const token = (yield* runCommand("gh", ["auth", "token"])).trim();
    const dir = yield* Effect.promise(() => Deno.makeTempDir({ prefix: "dogu-bench-" }));
    const memory = seedMemory(`${dir}/memory.jsonl`);
    yield* writePrivate(`${dir}/memory.jsonl`, memory.text);
    const memoryServer = {
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-memory"],
      env: { MEMORY_FILE_PATH: `${dir}/memory.jsonl` },
    };
    const hosted = JSON.parse(yield* Effect.promise(() => Deno.readTextFile(`${HERE}servers.json`))).mcpServers;
    // dogu's config keeps ${GITHUB_TOKEN} for dogu to fill in; Claude Code's gets the token itself.
    const servers = { mcpServers: { ...hosted, memory: memoryServer } };
    const withToken = JSON.parse(JSON.stringify(hosted).replaceAll("${GITHUB_TOKEN}", token));
    const direct = {
      mcpServers: {
        ...Object.fromEntries(Object.entries(withToken).map(([name, s]) => [name, { type: "http", ...(s as object) }])),
        memory: { type: "stdio", ...memoryServer },
      },
    };
    const root = `${dir}/mcp`;
    yield* writePrivate(`${dir}/servers.json`, JSON.stringify(servers));
    yield* writePrivate(`${dir}/direct.json`, JSON.stringify(direct));
    yield* writePrivate(
      `${dir}/serve.json`,
      JSON.stringify({ mcpServers: { tools: { command: "dogu", args: ["serve", "--root", root], env: { GITHUB_TOKEN: token } } } }),
    );
    yield* runCommand("dogu", ["sync", "--config", `${dir}/servers.json`, "--out", root], { env: { GITHUB_TOKEN: token } });
    const readme = yield* Effect.promise(() => Deno.readTextFile(`${root}/README.md`));
    const standing = `Your tools are in ${root}. This is its README:\n\n${readme}`;
    return { dir, root, directConfig: `${dir}/direct.json`, serveConfig: `${dir}/serve.json`, token, standing, seeded: memory.seeded };
  }),
  (ws) => Effect.tryPromise(() => Deno.remove(ws.dir, { recursive: true })).pipe(Effect.orElseSucceed(() => undefined)),
);

// --- The right answers, fetched live before any agent runs ---

const ctxOf = (ws: Workspace): Ctx => ({
  gh: (path) => runCommand("gh", ["api", path]).pipe(Effect.map((t) => JSON.parse(t))),
  tool: (path, args) => runCommand(`${ws.root}/${path}`, args, { env: { GITHUB_TOKEN: ws.token } }),
  seeded: ws.seeded,
});

const fetchFacts = (ws: Workspace) =>
  Effect.forEach(TASKS_RUN, (task) =>
    task.fetch(ctxOf(ws)).pipe(
      Effect.flatMap((facts) =>
        facts.length === 0 || facts.some((f) => f === "")
          ? Effect.fail(new BadTruth({ detail: `could not fetch the answer for ${task.id}` }))
          : Effect.succeed([task.id, facts] as const)
      ),
    )).pipe(Effect.map((pairs) => new Map<string, Facts>(pairs)));

// --- One run ---

/**
 * The Claude Code arguments for one run. `runId` starts the prompt so no two conversations are alike:
 * the standing part (system prompt, tool definitions, README) stays cached between runs, as it does
 * between a person's sessions, but no run can replay an earlier conversation from the cache.
 */
const argsFor = (setup: Setup, prompt: string, ws: Workspace, runId: string): ReadonlyArray<string> => {
  // "folder-read" gets the one line every agent can be given; "folder" has the README in its instructions.
  const told = setup === "folder-read" ? `Your tools are in ${ws.root}. Read ${ws.root}/README.md first. ` : "";
  const common = ["-p", `Run ${runId}. ${told}${prompt}`, "--model", MODEL, "--setting-sources", "project", "--strict-mcp-config"];
  const bash = ["--mcp-config", '{"mcpServers":{}}', "--tools", "Bash", "Read", "--allowedTools", "Bash", "Read"];
  const tools: Record<Setup, ReadonlyArray<string>> = {
    direct: ["--mcp-config", ws.directConfig, "--tools", "", "--allowedTools", ...MCP_SERVERS],
    // Claude Code's own answer to the same problem: tool definitions deferred, found with ToolSearch.
    "direct-search": ["--mcp-config", ws.directConfig, "--tools", "ToolSearch", "--allowedTools", "ToolSearch", ...MCP_SERVERS],
    folder: ["--append-system-prompt", ws.standing, ...bash],
    "folder-read": bash,
    codemode: ["--mcp-config", ws.serveConfig, "--tools", "", "--allowedTools", "mcp__tools__search", "mcp__tools__run_code"],
  };
  return [...common, ...tools[setup], "--output-format", "stream-json", "--verbose"];
};

const envFor = (setup: Setup, ws: Workspace): Record<string, string> =>
  setup === "direct-search" ? { GITHUB_TOKEN: ws.token, ENABLE_TOOL_SEARCH: "true" } : { GITHUB_TOKEN: ws.token };

const workDir = (ws: Workspace, setup: Setup) => `${ws.dir}/work-${setup}`;

/** One short session per setup before measuring, so every setup starts with its standing part cached. */
const warmUp = (ws: Workspace, setup: Setup) =>
  Effect.gen(function* () {
    yield* Effect.promise(() => Deno.mkdir(workDir(ws, setup), { recursive: true }));
    yield* runClaude(argsFor(setup, "Reply with just: ok", ws, "warmup"), {
      cwd: workDir(ws, setup),
      env: envFor(setup, ws),
      timeout: RUN_TIMEOUT,
    });
    yield* Console.log(`  warmed  ${setup}`);
  });

const runOne = (ws: Workspace, facts: Map<string, Facts>, setup: Setup, task: Task, rep: number) =>
  Effect.gen(function* () {
    // Every run of a setup starts in the same empty folder: no project files leak in, and like a person
    // working in one project, the session's standing instructions stay the same from run to run.
    const outcome = yield* Effect.result(
      runClaude(argsFor(setup, `${promptOf(task, ws.seeded)}${ANSWER_RULE}`, ws, crypto.randomUUID().slice(0, 8)), {
        cwd: workDir(ws, setup),
        env: envFor(setup, ws),
        timeout: RUN_TIMEOUT,
      }),
    );
    const base = { setup, task: task.id, suite: task.suite, rep };
    const record: RunRecord = Result.isSuccess(outcome)
      ? { ...base, correct: task.check(outcome.success.answer, facts.get(task.id) ?? []), metrics: outcome.success }
      : { ...base, correct: false, error: `${outcome.failure._tag}: ${"detail" in outcome.failure ? outcome.failure.detail : ""}` };
    yield* Console.log(
      `  ${record.correct ? "right" : "WRONG"}  ${setup.padEnd(13)} ${task.id.padEnd(14)} #${rep}  ${
        record.metrics ? `${record.metrics.totalInputTokens} tokens, ${record.metrics.seconds.toFixed(1)}s` : record.error
      }`,
    );
    return record;
  });

// --- The whole run ---

const section = (title: string, records: ReadonlyArray<RunRecord>, tasks: ReadonlyArray<Task>) =>
  records.length === 0 ? [] : [
    `## ${title}`,
    "",
    headline(records, SETUPS),
    "",
    summaryTable(records, SETUPS),
    "",
    "Median total input tokens per task:",
    "",
    taskTable(records, SETUPS, tasks),
    "",
  ];

const program = Effect.scoped(
  Effect.gen(function* () {
    const ws = yield* workspace;
    const facts = yield* fetchFacts(ws);
    yield* Console.log(`Answers: ${JSON.stringify(Object.fromEntries(facts))}`);
    if (env("BENCH_DRY", "") === "1") {
      const size = (yield* Effect.promise(() => Deno.readTextFile(`${ws.root}/README.md`))).length;
      return yield* Console.log(`Dry run: ${TASKS_RUN.length} tasks, README ${size} bytes. No agents run.`);
    }
    // Interleave setups so a slow minute on a server hits all of them alike.
    const plan = Array.from({ length: REPS }, (_, i) => i + 1).flatMap((rep) =>
      TASKS_RUN.flatMap((task) => SETUPS.map((setup) => ({ setup, task, rep })))
    );
    yield* Effect.forEach(SETUPS, (setup) => warmUp(ws, setup), { concurrency: SETUPS.length });
    yield* Console.log(`Running ${plan.length} runs, ${CONCURRENCY} at a time, model ${MODEL}`);
    const records = yield* Effect.forEach(plan, (p) => runOne(ws, facts, p.setup, p.task, p.rep), {
      concurrency: CONCURRENCY,
    });

    const date = new Date().toISOString().slice(0, 10);
    const dogu = (yield* runCommand("dogu", ["--version"])).trim();
    const claude = (yield* runCommand("claude", ["--version"])).trim();
    const commit = (yield* runCommand("git", ["-C", HERE, "rev-parse", "--short", "HEAD"])).trim();
    const dirty = (yield* runCommand("git", ["-C", HERE, "status", "--porcelain", "--", "."])).trim() !== "";
    const tasksOf = (suite: string) => TASKS_RUN.filter((t) => t.suite === suite);
    const report = [
      `# Benchmark ${date}`,
      "",
      `${dogu}, Claude Code ${claude}, model \`${MODEL}\`, ${REPS} run${REPS === 1 ? "" : "s"} of each task per setup, ` +
      `bench at commit ${commit}${dirty ? " with uncommitted changes" : ""}. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).`,
      "",
      ...section("Held-out tasks (written before any run, never tuned against)", records.filter((r) => r.suite === "heldout"), tasksOf("heldout")),
      ...section("Tuning tasks (dogu was tuned while watching these)", records.filter((r) => r.suite === "tuning"), tasksOf("tuning")),
    ].join("\n");
    const out = `${HERE}results/${env("BENCH_OUT", date)}`;
    // Commands and answers name the temp folder, whose path can hold your user name. Saved results say <work>.
    const real = yield* Effect.promise(() => Deno.realPath(ws.dir));
    const scrub = (text: string) => text.replaceAll(ws.dir, "<work>").replaceAll(real, "<work>");
    yield* Effect.promise(() => Deno.writeTextFile(`${out}.md`, scrub(report)));
    const json = JSON.stringify({ date, dogu, claude, model: MODEL, commit, dirty, answers: Object.fromEntries(facts), records }, null, 2);
    yield* Effect.promise(() => Deno.writeTextFile(`${out}.json`, scrub(json)));
    yield* Console.log(`\n${scrub(report)}\nWrote ${out}.md and ${out}.json`);
  }),
);

Effect.runPromise(program).catch((e) => {
  console.error("benchmark failed:", e);
  Deno.exit(1);
});
