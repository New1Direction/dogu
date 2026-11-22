// dogu benchmark: the same tasks, done by Claude Code with the tools given three ways.
//
//   deno run --allow-run --allow-env --allow-read --allow-write --allow-net bench/run.ts
//
// Needs: claude (logged in), gh (logged in), dogu and Deno. See bench/README.md.
// Settings: BENCH_REPS (3), BENCH_CONCURRENCY (3), BENCH_MODEL (sonnet), BENCH_TASKS and BENCH_SETUPS (all, comma lists),
// BENCH_OUT (a name for the results files instead of the date).

import { Console, Data, Duration, Effect, Result } from "npm:effect@4.1.5";
import { CommandFailed, runClaude, runCommand } from "./claude.ts";
import { headline, type RunRecord, SETUP_LABELS, summaryTable, taskTable } from "./report.ts";
import { type Task, TASKS, type Truth } from "./tasks.ts";

class BadTruth extends Data.TaggedError("BadTruth")<{ readonly detail: string }> {}

const env = (name: string, fallback: string) => Deno.env.get(name) ?? fallback;
const REPS = Number(env("BENCH_REPS", "3"));
const CONCURRENCY = Number(env("BENCH_CONCURRENCY", "3"));
const MODEL = env("BENCH_MODEL", "sonnet");
const ONLY = env("BENCH_TASKS", "").split(",").filter((t) => t !== "");
const RUN_TIMEOUT = Duration.minutes(5);
const HERE = new URL(".", import.meta.url).pathname;
const ALL_SETUPS = ["direct", "folder", "folder-read", "codemode"] as const;
type Setup = (typeof ALL_SETUPS)[number];
const ONLY_SETUPS = env("BENCH_SETUPS", "").split(",").filter((s) => s !== "");
const SETUPS: ReadonlyArray<Setup> = ALL_SETUPS.filter((s) => ONLY_SETUPS.length === 0 || ONLY_SETUPS.includes(s));

const ANSWER_RULE = " Use only the tools you have been given. Answer in under 6 lines.";

interface Workspace {
  readonly dir: string;
  readonly root: string; // the dogu folder
  readonly directConfig: string; // Claude Code config with the three servers, token included
  readonly serveConfig: string; // Claude Code config with only `dogu serve`
  readonly token: string;
  /** The folder's README as standing instructions: what `@~/mcp/README.md` in CLAUDE.md, or pasting it into AGENTS.md, gives. */
  readonly standing: string;
}

// --- Setup and cleanup ---

const writePrivate = (path: string, value: unknown) =>
  Effect.tryPromise({
    try: () => Deno.writeTextFile(path, JSON.stringify(value), { mode: 0o600 }),
    catch: (e) => new CommandFailed({ command: `write ${path}`, detail: String(e) }),
  });

/** A temp folder with the configs and a synced dogu folder. Removed when the run ends, even on failure. */
const workspace = Effect.acquireRelease(
  Effect.gen(function* () {
    const token = (yield* runCommand("gh", ["auth", "token"])).trim();
    const dir = yield* Effect.promise(() => Deno.makeTempDir({ prefix: "dogu-bench-" }));
    const servers = JSON.parse(yield* Effect.promise(() => Deno.readTextFile(`${HERE}servers.json`)));
    const withToken = JSON.parse(JSON.stringify(servers).replaceAll("${GITHUB_TOKEN}", token));
    const direct = {
      mcpServers: Object.fromEntries(
        Object.entries(withToken.mcpServers).map(([name, s]) => [name, { type: "http", ...(s as object) }]),
      ),
    };
    const root = `${dir}/mcp`;
    yield* writePrivate(`${dir}/direct.json`, direct);
    yield* writePrivate(`${dir}/serve.json`, {
      mcpServers: { tools: { command: "dogu", args: ["serve", "--root", root], env: { GITHUB_TOKEN: token } } },
    });
    yield* runCommand("dogu", ["sync", "--config", `${HERE}servers.json`, "--out", root], {
      env: { GITHUB_TOKEN: token },
    });
    const readme = yield* Effect.promise(() => Deno.readTextFile(`${root}/README.md`));
    const standing = `Your tools are in ${root}. This is its README:\n\n${readme}`;
    return { dir, root, directConfig: `${dir}/direct.json`, serveConfig: `${dir}/serve.json`, token, standing };
  }),
  (ws) =>
    Effect.tryPromise(() => Deno.remove(ws.dir, { recursive: true })).pipe(Effect.orElseSucceed(() => undefined)),
);

// --- The right answers, fetched live ---

const gh = (path: string) => runCommand("gh", ["api", path]).pipe(Effect.map((t) => JSON.parse(t)));

const fetchTruth = (ws: Workspace) =>
  Effect.gen(function* () {
    const [release, search] = yield* Effect.all([
      gh("repos/modelcontextprotocol/typescript-sdk/releases/latest"),
      gh("search/repositories?q=org:modelcontextprotocol&sort=stars&per_page=3"),
    ]);
    const top: ReadonlyArray<{ name: string; open_issues_count: number }> = search.items;
    const issuesOnly = yield* Effect.forEach(top, (repo) =>
      gh(`search/issues?q=repo:modelcontextprotocol/${repo.name}+is:issue+is:open&per_page=1`).pipe(
        Effect.map((r) => ({ name: repo.name, count: r.total_count as number })),
      ));
    const most = (xs: ReadonlyArray<{ name: string; count: number }>) =>
      xs.reduce((a, b) => (b.count > a.count ? b : a)).name;
    const tool = (path: string, args: ReadonlyArray<string>) =>
      runCommand(`${ws.root}/${path}`, args, { env: { GITHUB_TOKEN: ws.token } });
    const structure = yield* tool("deepwiki/read_wiki_structure", ["--repoName", `modelcontextprotocol/${top[0].name}`]);
    const libraries = yield* tool("context7/resolve-library-id", ["--libraryName", "effect", "--query", "Effect TypeScript library"]);
    const truth: Truth = {
      latestTag: release.tag_name,
      top3: top.map((r) => r.name),
      sections: structure.split("\n").filter((l) => l.startsWith("- ")).map((l) => l.replace(/^- [\d.]+\s*/, "")),
      context7Ids: [...libraries.matchAll(/library ID:\s*(\S+)/g)].map((m) => m[1]),
      mostIssues: [...new Set([most(top.map((r) => ({ name: r.name, count: r.open_issues_count }))), most(issuesOnly)])],
    };
    const empty = Object.entries(truth).filter(([, v]) => (Array.isArray(v) ? v.length === 0 : !v));
    return empty.length > 0
      ? yield* Effect.fail(new BadTruth({ detail: `could not fetch: ${empty.map(([k]) => k).join(", ")}` }))
      : truth;
  });

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
  const tools = {
    direct: ["--mcp-config", ws.directConfig, "--tools", "", "--allowedTools", "mcp__github", "mcp__context7", "mcp__deepwiki"],
    folder: [
      "--append-system-prompt", ws.standing,
      "--mcp-config", '{"mcpServers":{}}', "--tools", "Bash", "Read", "--allowedTools", "Bash", "Read",
    ],
    "folder-read": ["--mcp-config", '{"mcpServers":{}}', "--tools", "Bash", "Read", "--allowedTools", "Bash", "Read"],
    codemode: ["--mcp-config", ws.serveConfig, "--tools", "", "--allowedTools", "mcp__tools__search", "mcp__tools__run_code"],
  }[setup];
  return [...common, ...tools, "--output-format", "stream-json", "--verbose"];
};

const workDir = (ws: Workspace, setup: Setup) => `${ws.dir}/work-${setup}`;

/** One short session per setup before measuring, so every setup starts with its standing part cached. */
const warmUp = (ws: Workspace, setup: Setup) =>
  Effect.gen(function* () {
    yield* Effect.promise(() => Deno.mkdir(workDir(ws, setup), { recursive: true }));

    yield* runClaude(argsFor(setup, "Reply with just: ok", ws, "warmup"), {
      cwd: workDir(ws, setup),
      env: { GITHUB_TOKEN: ws.token },
      timeout: RUN_TIMEOUT,
    });
    yield* Console.log(`  warmed  ${setup}`);
  });

const runOne = (ws: Workspace, truth: Truth, setup: Setup, task: Task, rep: number) =>
  Effect.gen(function* () {
    // Every run of a setup starts in the same empty folder: no project files leak in, and like a person
    // working in one project, the session's standing instructions stay the same from run to run.
    const cwd = workDir(ws, setup);
    const outcome = yield* Effect.result(
      runClaude(argsFor(setup, `${task.prompt}${ANSWER_RULE}`, ws, crypto.randomUUID().slice(0, 8)), {
        cwd,
        env: { GITHUB_TOKEN: ws.token },
        timeout: RUN_TIMEOUT,
      }),
    );
    const record: RunRecord = Result.isSuccess(outcome)
      ? { setup, task: task.id, rep, correct: task.check(outcome.success.answer, truth), metrics: outcome.success }
      : { setup, task: task.id, rep, correct: false, error: `${outcome.failure._tag}: ${"detail" in outcome.failure ? outcome.failure.detail : ""}` };
    yield* Console.log(
      `  ${record.correct ? "right" : "WRONG"}  ${setup.padEnd(8)} ${task.id.padEnd(8)} #${rep}  ${
        record.metrics ? `${record.metrics.totalInputTokens} tokens, ${record.metrics.seconds.toFixed(1)}s` : record.error
      }`,
    );
    return record;
  });

// --- The whole run ---

const program = Effect.scoped(
  Effect.gen(function* () {
    const ws = yield* workspace;
    const truth = yield* fetchTruth(ws);
    yield* Console.log(`Truth: ${JSON.stringify(truth)}`);
    const tasks = TASKS.filter((t) => ONLY.length === 0 || ONLY.includes(t.id));
    // Interleave setups so a slow minute on a server hits all of them alike.
    const plan = Array.from({ length: REPS }, (_, i) => i + 1).flatMap((rep) =>
      tasks.flatMap((task) => SETUPS.map((setup) => ({ setup, task, rep })))
    );
    yield* Effect.forEach(SETUPS, (setup) => warmUp(ws, setup), { concurrency: SETUPS.length });
    yield* Console.log(`Running ${plan.length} runs, ${CONCURRENCY} at a time, model ${MODEL}`);
    const records = yield* Effect.forEach(plan, (p) => runOne(ws, truth, p.setup, p.task, p.rep), {
      concurrency: CONCURRENCY,
    });

    const date = new Date().toISOString().slice(0, 10);
    const dogu = (yield* runCommand("dogu", ["--version"])).trim();
    const claude = (yield* runCommand("claude", ["--version"])).trim();
    const report = [
      `# Benchmark ${date}`,
      "",
      `${dogu}, Claude Code ${claude}, model \`${MODEL}\`, ${REPS} run${REPS === 1 ? "" : "s"} of each task per setup. Servers: GitHub (official, hosted), Context7, DeepWiki.`,
      "",
      headline(records, SETUPS),
      "",
      summaryTable(records, SETUPS),
      "",
      "Median total input tokens per task:",
      "",
      taskTable(records, SETUPS, tasks),
      "",
    ].join("\n");
    const out = `${HERE}results/${env("BENCH_OUT", date)}`;
    yield* Effect.promise(() => Deno.writeTextFile(`${out}.md`, report));
    // Commands and answers name the temp folder, whose path can hold your user name. Saved results say <work>.
    const real = yield* Effect.promise(() => Deno.realPath(ws.dir));
    const json = JSON.stringify({ date, dogu, claude, model: MODEL, truth, records }, null, 2)
      .replaceAll(ws.dir, "<work>")
      .replaceAll(real, "<work>");
    yield* Effect.promise(() => Deno.writeTextFile(`${out}.json`, json));
    yield* Console.log(`\n${report}\nWrote ${out}.md and ${out}.json`);
    yield* Console.log(`(setups: ${Object.values(SETUP_LABELS).join(", ")})`);
  }),
);

Effect.runPromise(program).catch((e) => {
  console.error("benchmark failed:", e);
  Deno.exit(1);
});
