// MCPMark's filesystem tasks (https://github.com/eval-sys/mcpmark, Apache-2.0), run on dogu's code mode
// and on the server loaded directly. A public benchmark we did not write: its prompts, its starting files
// and its own checkers (verify.py, fixed expected values, no LLM judge) decide what is right.
//
// It is run the way MCPMark runs it: the server it pins, a fresh copy of the task's files for every run
// (timestamps kept: some tasks are about them), the prompt it builds, and its verify.py on the result.
// Claude Code's own file and shell tools are off in both setups, so all work goes through MCP. dogu's
// folder is not run: it needs a shell, and a shell reaches the files without the server.
//
//   git clone https://github.com/eval-sys/mcpmark && git -C mcpmark checkout cd45b7f
//   MCPMARK_DIR=./mcpmark deno run -A bench/mcpmark.ts
//
// Settings: MCPMARK_DIR (required), MCPMARK_TASKS (all, easy, standard, or a comma list of
// level/category/task), BENCH_SETUPS (direct,codemode), BENCH_REPS (3), BENCH_CONCURRENCY (3),
// BENCH_MODEL (sonnet), BENCH_OUT (results file name), DOGU_BIN (dogu), BENCH_STREAMS (a folder to save
// each run's whole event stream in, to see what every tool returned), DOGU_B_BIN (a candidate dogu build,
// run as the setup codemode-b, for a same-run A/B).
//
// MCPMARK_DIR can also be a folder of tasks in MCPMark's layout that are not MCPMark's, such as
// bench/fs-tasks: a task's meta.json may give its starting files as "local:<name>", the folder
// <MCPMARK_DIR>/states/<name>.

import { Duration, Effect } from "npm:effect@4.1.5";
import { runClaude, runCommand } from "./claude.ts";
import { planOf } from "./plan.ts";
import { headline, type RunRecord, summaryTable, timeTable } from "./report.ts";

const env = (name: string, fallback: string) => Deno.env.get(name) ?? fallback;
const MCPMARK = Deno.env.get("MCPMARK_DIR");
if (!MCPMARK) throw new Error("set MCPMARK_DIR to a checkout of https://github.com/eval-sys/mcpmark");
const PINNED_COMMIT = "cd45b7f";
/** The server MCPMark pins (src/agents/base_agent.py). */
const SERVER = "@modelcontextprotocol/server-filesystem@2025.12.18";
/** The line MCPMark adds to every task's description (src/base/task_manager.py). */
const NOTE = "\n\nNote: Based on your understanding, solve the task all at once by yourself, don't ask for my opinions on anything.";
type Setup = "direct" | "codemode" | "codemode-b";
const SETUPS = env("BENCH_SETUPS", "direct,codemode").split(",") as ReadonlyArray<Setup>;
const REPS = Number(env("BENCH_REPS", "3"));
const CONCURRENCY = Number(env("BENCH_CONCURRENCY", "3"));
const MODEL = env("BENCH_MODEL", "sonnet");
const DOGU = env("DOGU_BIN", "dogu");
const DOGU_B = Deno.env.get("DOGU_B_BIN");
const FILTER = env("MCPMARK_TASKS", "all");
const RUN_TIMEOUT = Duration.minutes(10);
const STREAMS = Deno.env.get("BENCH_STREAMS");
const HERE = new URL(".", import.meta.url).pathname;

interface MarkTask {
  readonly id: string; // level/category/task
  readonly level: string;
  readonly category: string;
  readonly dir: string;
  readonly stateUrl: string;
}

/** Every filesystem task with a description, a checker and a starting state. */
const discover = (): ReadonlyArray<MarkTask> => {
  const root = `${MCPMARK}/tasks/filesystem`;
  const dirs = (p: string) => [...Deno.readDirSync(p)].filter((e) => e.isDirectory).map((e) => e.name).sort();
  return dirs(root).flatMap((level) =>
    dirs(`${root}/${level}`).flatMap((category) =>
      dirs(`${root}/${level}/${category}`).flatMap((task) => {
        const dir = `${root}/${level}/${category}/${task}`;
        const has = (f: string) => { try { return Deno.statSync(`${dir}/${f}`).isFile; } catch { return false; } };
        if (!has("description.md") || !has("verify.py") || !has("meta.json")) return [];
        const meta = JSON.parse(Deno.readTextFileSync(`${dir}/meta.json`));
        const stateUrl = String(meta?.meta_data?.stateUrl ?? "");
        return stateUrl ? [{ id: `${level}/${category}/${task}`, level, category, dir, stateUrl }] : [];
      })
    )
  );
};

const chosen = (all: ReadonlyArray<MarkTask>) =>
  FILTER === "all" ? all
  : FILTER === "easy" || FILTER === "standard" ? all.filter((t) => t.level === FILTER)
  : all.filter((t) => FILTER.split(",").includes(t.id));

/** Downloads and unpacks each starting state once. The zips hold one folder named like the state. */
const unpack = (cache: string, url: string) =>
  Effect.gen(function* () {
    if (url.startsWith("local:")) return `${MCPMARK}/states/${url.slice("local:".length)}`;
    const name = new URL(url).pathname.split("/").pop()!.replace(/\.zip$/, "");
    const out = `${cache}/${name}`;
    const exists = yield* Effect.promise(() => Deno.stat(`${out}/${name}`).then(() => true, () => false));
    if (exists) return `${out}/${name}`;
    yield* Effect.promise(() => Deno.mkdir(out, { recursive: true }));
    yield* runCommand("curl", ["-fsSL", "-o", `${out}.zip`, url]);
    yield* runCommand("unzip", ["-q", "-o", `${out}.zip`, "-d", out]);
    yield* Effect.promise(() => Deno.remove(`${out}/__MACOSX`, { recursive: true }).catch(() => {}));
    return `${out}/${name}`;
  });

/**
 * A run's own copy of the starting files (timestamps kept), its server config, and dogu's folder.
 * Claude Code starts in the copy: the tasks say "the current directory" for it, and Claude Code tells
 * the model where it started. (Started anywhere else, agents looked there and found nothing.) Its own
 * file and shell tools are off, so starting there gives no way around the server.
 */
const prepare = (state: string, runDir: string, setup: Setup) =>
  Effect.gen(function* () {
    const testDir = `${runDir}/${state.split("/").pop()}`;
    yield* Effect.promise(() => Deno.mkdir(runDir, { recursive: true }));
    yield* runCommand("cp", ["-Rp", state, testDir]);
    const cwd = testDir;
    const server = { command: "npx", args: ["-y", SERVER, testDir] };
    if (setup === "direct") {
      const config = `${runDir}/direct.json`;
      yield* Effect.promise(() => Deno.writeTextFile(config, JSON.stringify({ mcpServers: { filesystem: { type: "stdio", ...server } } })));
      return { testDir, cwd, tools: ["--mcp-config", config, "--tools", "", "--allowedTools", "mcp__filesystem"] };
    }
    const root = `${runDir}/mcp`;
    yield* Effect.promise(() => Deno.writeTextFile(`${runDir}/servers.json`, JSON.stringify({ mcpServers: { filesystem: server } })));
    const dogu = setup === "codemode-b" ? DOGU_B : DOGU;
    if (!dogu) throw new Error("codemode-b needs DOGU_B_BIN");
    yield* runCommand(dogu, ["sync", "--config", `${runDir}/servers.json`, "--out", root]);
    const config = `${runDir}/serve.json`;
    yield* Effect.promise(() => Deno.writeTextFile(config, JSON.stringify({ mcpServers: { tools: { command: dogu, args: ["serve", "--root", root] } } })));
    const tools = ["--mcp-config", config, "--tools", "", "--allowedTools", "mcp__tools__search", "mcp__tools__run_code", "mcp__tools__forge"];
    return { testDir, cwd, tools };
  });

/** MCPMark's checker on the run's files: exit 0 is a pass. Standard library only (checked before use). */
const verify = (task: MarkTask, testDir: string) =>
  Effect.tryPromise(() =>
    new Deno.Command("python3", {
      args: ["-I", `${task.dir}/verify.py`],
      env: { FILESYSTEM_TEST_DIR: testDir, PATH: "/usr/bin:/bin" },
      clearEnv: true,
      stdout: "piped",
      stderr: "piped",
    }).output()
  ).pipe(
    Effect.map((o) => ({ pass: o.success, output: new TextDecoder().decode(o.stdout).slice(-1500) })),
    Effect.orElseSucceed(() => ({ pass: false, output: "verify.py did not run" })),
  );

const runOne = (task: MarkTask, setup: Setup, rep: number, work: string, cache: string) =>
  Effect.gen(function* () {
    const runId = `${task.id.replaceAll("/", "-")}-${setup}-${rep}-${crypto.randomUUID().slice(0, 8)}`;
    const runDir = `${work}/${runId}`;
    const state = yield* unpack(cache, task.stateUrl);
    const { testDir, cwd, tools } = yield* prepare(state, runDir, setup);
    const prompt = `Run ${runId}. ${Deno.readTextFileSync(`${task.dir}/description.md`)}${NOTE}`;
    const args = ["-p", prompt, "--model", MODEL, "--setting-sources", "project", "--strict-mcp-config", ...tools, "--output-format", "stream-json", "--verbose"];
    const saveStreamTo = STREAMS ? `${STREAMS}/${runId}.jsonl` : undefined;
    const metrics = yield* runClaude(args, { cwd, env: {}, timeout: RUN_TIMEOUT, saveStreamTo });
    const checked = yield* verify(task, testDir);
    console.error(`${checked.pass ? "PASS" : "fail"} ${setup} ${task.id}#${rep} ${metrics.seconds.toFixed(1)}s $${metrics.costUsd?.toFixed(4)}`);
    return { setup, task: task.id, suite: task.level, rep, correct: checked.pass, metrics, check: checked.output } satisfies RunRecord;
  }).pipe(
    Effect.catch((e: unknown) =>
      Effect.succeed({ setup, task: task.id, suite: task.level, rep, correct: false, error: String((e as { detail?: string }).detail ?? e).slice(0, 300) } satisfies RunRecord)
    ),
  );

const main = Effect.gen(function* () {
  const head = (yield* runCommand("git", ["-C", MCPMARK, "rev-parse", "--short", "HEAD"]).pipe(Effect.orElseSucceed(() => "local"))).trim();
  if (!head.startsWith(PINNED_COMMIT)) console.error(`note: MCPMark is at ${head}, results were taken at ${PINNED_COMMIT}`);
  const tasks = chosen(discover());
  if (tasks.length === 0) throw new Error(`no MCPMark filesystem tasks match ${FILTER}`);
  const work = yield* Effect.promise(() => Deno.makeTempDir({ prefix: "dogu-mcpmark-" }));
  const cache = `${HERE}../.mcpmark-cache`;
  // Unpack every state first, one at a time, so runs never race on a download.
  yield* Effect.forEach([...new Set(tasks.map((t) => t.stateUrl))], (url) => unpack(cache, url), { discard: true });
  const plan = planOf(REPS, tasks, SETUPS);
  console.error(`${tasks.length} tasks x ${SETUPS.length} setups x ${REPS} = ${plan.length} runs, ${MODEL}`);
  const records = yield* Effect.forEach(plan, ({ task, setup, rep }) => runOne(task, setup, rep, work, cache), { concurrency: CONCURRENCY });
  const out = env("BENCH_OUT", `${new Date().toISOString().slice(0, 10)}-mcpmark-filesystem`);
  const md = [
    `# MCPMark filesystem tasks: dogu code mode against the server loaded directly`,
    "",
    `MCPMark ${head} (eval-sys/mcpmark, Apache-2.0), ${tasks.length} filesystem tasks, ${REPS} runs each, Claude Code with ${MODEL}. ` +
    `Right means MCPMark's own verify.py passed. Server: ${SERVER}. Claude Code's own file and shell tools off in both setups.`,
    "",
    headline(records, SETUPS),
    "",
    summaryTable(records, SETUPS),
    "",
    timeTable(records, SETUPS),
    "",
    "| Task | " + SETUPS.join(" | ") + " |",
    "|---|" + SETUPS.map(() => "---").join("|") + "|",
    ...tasks.map((t) =>
      `| ${t.id} | ` + SETUPS.map((s) => {
        const rs = records.filter((r) => r.task === t.id && r.setup === s);
        return `${rs.filter((r) => r.correct).length}/${rs.length}`;
      }).join(" | ") + " |"
    ),
  ].join("\n");
  yield* Effect.promise(() => Deno.writeTextFile(`${HERE}results/${out}.md`, md + "\n"));
  yield* Effect.promise(() => Deno.writeTextFile(`${HERE}results/${out}.json`, JSON.stringify({ mcpmark: head, model: MODEL, server: SERVER, records }, null, 1)));
  console.log(md);
  yield* Effect.promise(() => Deno.remove(work, { recursive: true }).catch(() => {}));
});

Effect.runPromise(main).catch((e) => {
  console.error(e);
  Deno.exit(1);
});
