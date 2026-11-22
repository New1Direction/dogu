// dogu benchmark: the same tasks, done by Claude Code with the tools given five ways.
//
//   deno run --allow-run --allow-env --allow-read --allow-write --allow-net bench/run.ts
//
// Needs: claude (logged in), gh (logged in), dogu, Node (for the local memory server) and Deno.
// Settings: BENCH_REPS (3), BENCH_CONCURRENCY (3), BENCH_MODEL (sonnet), BENCH_SUITE (all, tuning or heldout),
// BENCH_TASKS and BENCH_SETUPS (all, comma lists), BENCH_OUT (a name for the results files instead of the date),
// BENCH_DRY=1 (fetch and print the right answers, run no agents), CODEX_FAST=1 (Codex's fast mode).

import { Console, Data, Effect, Result } from "npm:effect@4.1.5";
import { runAgent, runCommand } from "./claude.ts";
import { ALL_SETUPS, ANSWER_RULE, argsFor, envFor, makeWorkspace, RUN_TIMEOUT, type Setup, warmUp, workDir, type Workspace } from "./workspace.ts";
import { planOf } from "./plan.ts";
import { headline, type RunRecord, summaryTable, taskTable, timeTable } from "./report.ts";
import { checkTheCheck, type Ctx, type Facts, promptOf, type Task, TASKS } from "./tasks.ts";

class BadTruth extends Data.TaggedError("BadTruth")<{ readonly detail: string }> {}

const env = (name: string, fallback: string) => Deno.env.get(name) ?? fallback;
const list = (name: string) => env(name, "").split(",").filter((s) => s !== "");
const REPS = Number(env("BENCH_REPS", "3"));
const CONCURRENCY = Number(env("BENCH_CONCURRENCY", "3"));
const MODEL = env("BENCH_MODEL", "sonnet");
const SUITE = env("BENCH_SUITE", "all");
const HERE = new URL(".", import.meta.url).pathname;
const SETUPS: ReadonlyArray<Setup> = ALL_SETUPS.filter((s) => list("BENCH_SETUPS").length === 0 || list("BENCH_SETUPS").includes(s));
const TASKS_RUN = TASKS.filter((t) =>
  (SUITE === "all" || t.suite === SUITE) && (list("BENCH_TASKS").length === 0 || list("BENCH_TASKS").includes(t.id))
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
          // Test each check on the right answer before any agent runs.
          : checkTheCheck(task, facts) !== null
          ? Effect.fail(new BadTruth({ detail: checkTheCheck(task, facts)! }))
          : Effect.succeed([task.id, facts] as const)
      ),
    )).pipe(Effect.map((pairs) => new Map<string, Facts>(pairs)));

// --- One run ---

const runOne = (ws: Workspace, facts: Map<string, Facts>, setup: Setup, task: Task, rep: number) =>
  Effect.gen(function* () {
    // Every run of a setup starts in the same empty folder: no project files leak in, and like a person
    // working in one project, the session's standing instructions stay the same from run to run.
    const call = yield* argsFor(setup, `${promptOf(task, ws.seeded)}${ANSWER_RULE}`, ws, crypto.randomUUID().slice(0, 8), MODEL);
    const outcome = yield* Effect.result(
      runAgent(call.agent, call.bin, call.args, {
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
    "Where the time went:",
    "",
    timeTable(records, SETUPS),
    "",
    "Median total input tokens per task:",
    "",
    taskTable(records, SETUPS, tasks),
    "",
  ];

const program = Effect.scoped(
  Effect.gen(function* () {
    const ws = yield* makeWorkspace({ memory: TASKS_RUN.some((t) => ["memory-find", "memory-years", "started-in", "entity-year"].includes(t.id)) });
    const facts = yield* fetchFacts(ws);
    yield* Console.log(`Answers: ${JSON.stringify(Object.fromEntries(facts))}`);
    if (env("BENCH_DRY", "") === "1") {
      const size = (yield* Effect.promise(() => Deno.readTextFile(`${ws.root}/README.md`))).length;
      return yield* Console.log(`Dry run: ${TASKS_RUN.length} tasks, README ${size} bytes. No agents run.`);
    }
    // Interleave setups so a slow minute on a server hits all of them alike, and rotate who goes first.
    const plan = planOf(REPS, TASKS_RUN, SETUPS);
    yield* Effect.forEach(SETUPS, (setup) => warmUp(ws, setup, MODEL), { concurrency: SETUPS.length });
    yield* Console.log(`Running ${plan.length} runs, ${CONCURRENCY} at a time, model ${MODEL}`);
    const records = yield* Effect.forEach(plan, (p) => runOne(ws, facts, p.setup, p.task, p.rep), {
      concurrency: CONCURRENCY,
    });

    const date = new Date().toISOString().slice(0, 10);
    const dogu = (yield* runCommand("dogu", ["--version"])).trim();
    const claude = (yield* runCommand("claude", ["--version"])).trim();
    const commit = (yield* runCommand("git", ["-C", HERE, "rev-parse", "--short", "HEAD"])).trim();
    // Uncommitted changes to the bench itself; earlier results in bench/results do not count.
    const dirty = (yield* runCommand("git", ["-C", HERE, "status", "--porcelain", "--", ".", ":!results"])).trim() !== "";
    const tasksOf = (suite: string) => TASKS_RUN.filter((t) => t.suite === suite);
    const report = [
      `# Benchmark ${date}`,
      "",
      `${dogu}, Claude Code ${claude}, model \`${MODEL}\`, ${REPS} run${REPS === 1 ? "" : "s"} of each task per setup, ` +
      `bench at commit ${commit}${dirty ? " with uncommitted changes" : ""}. Servers: GitHub (official, hosted), Context7, DeepWiki, and the memory server (local).`,
      "",
      ...section("Held-out tasks (written before any run, never tuned against)", records.filter((r) => r.suite === "heldout"), tasksOf("heldout")),
      ...section("Held-out tasks, set 2 (frozen before the code-mode speed work)", records.filter((r) => r.suite === "heldout2"), tasksOf("heldout2")),
      ...section("Held-out tasks, set 3 (frozen before the work on calls made together)", records.filter((r) => r.suite === "heldout3"), tasksOf("heldout3")),
      ...section("Held-out tasks, set 4 (frozen before the change about printing whole results)", records.filter((r) => r.suite === "heldout4"), tasksOf("heldout4")),
      ...section("Held-out tasks, set 5 (frozen before the accuracy fix)", records.filter((r) => r.suite === "heldout5"), tasksOf("heldout5")),
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
