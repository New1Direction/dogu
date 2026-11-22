// dogu forge benchmark: does saving a multi-step program as a tool pay off over repeated jobs?
//
//   deno run --allow-run --allow-env --allow-read --allow-write --allow-net bench/forge.ts
//
// One job family, five sessions in a row with a different org each time. Two conditions, each in its
// own fresh folder so forged tools never leak between them:
//   forge     session 1 is asked to save its program as a tool; sessions 2-5 get the plain job
//   no-forge  sessions 1-5 get the plain job
// Settings: FORGE_WORLDS (3 per setup and condition), FORGE_CONCURRENCY (3), BENCH_MODEL (sonnet),
// BENCH_SETUPS (folder,codemode), BENCH_OUT.

import { Console, Effect, Result } from "npm:effect@4.1.5";
import { runClaude, runCommand, type RunMetrics } from "./claude.ts";
import { ANSWER_RULE, argsFor, envFor, makeWorkspace, RUN_TIMEOUT, type Setup, warmUp, workDir, type Workspace } from "./workspace.ts";

const env = (name: string, fallback: string) => Deno.env.get(name) ?? fallback;
const WORLDS = Number(env("FORGE_WORLDS", "3"));
const CONCURRENCY = Number(env("FORGE_CONCURRENCY", "3"));
const MODEL = env("BENCH_MODEL", "sonnet");
const SETUPS = env("BENCH_SETUPS", "folder,codemode").split(",") as Setup[];
const ORGS = ["tokio-rs", "denoland", "vercel", "rust-lang", "modelcontextprotocol"];
const CONDITIONS = ["forge", "no-forge"] as const;
type Condition = (typeof CONDITIONS)[number];
const HERE = new URL(".", import.meta.url).pathname;

const job = (org: string) =>
  `For each of the 3 most-starred repos in the ${org} GitHub org, what is its latest release tag? Say 'none' for a repo with no releases.`;

const forgeAsk: Record<string, string> = {
  folder: " Then save your program as a tool with ./forge (the README says how), so next time this job is one call.",
  codemode: " Then save your program as a tool with the forge tool, so next time this job is one call.",
};

/** The right answer for one org: each top-3 repo's latest release tag, or its name if it has none. */
const truthOf = (org: string) =>
  Effect.gen(function* () {
    const gh = (path: string) => runCommand("gh", ["api", path]).pipe(Effect.map((t) => JSON.parse(t)));
    const top = (yield* gh(`search/repositories?q=org:${org}&sort=stars&per_page=3`)).items as Array<{ name: string; full_name: string }>;
    return yield* Effect.forEach(top, (r) =>
      gh(`repos/${r.full_name}/releases/latest`).pipe(
        Effect.map((rel) => ({ repo: r.name, tag: String(rel.tag_name) })),
        Effect.orElseSucceed(() => ({ repo: r.name, tag: "" })),
      ));
  });

type Truth = ReadonlyArray<{ repo: string; tag: string }>;
const correct = (answer: string, truth: Truth) =>
  truth.every(({ repo, tag }) => answer.toLowerCase().includes((tag === "" ? repo : tag.replace(/^v/, "")).toLowerCase()));

interface Session {
  readonly setup: Setup;
  readonly condition: Condition;
  readonly world: number;
  readonly session: number;
  readonly org: string;
  readonly correct: boolean;
  /** The session called a forged tool. */
  readonly usedForged: boolean;
  readonly metrics?: RunMetrics;
  readonly error?: string;
}

const usedForged = (m: RunMetrics) => m.steps.some((s) => /forged\/|tools\.forged\.|mcp__tools__forge/.test(s) && !/\.\/forge |mcp__tools__forge /.test(s));

/** One world: a fresh folder, then the five sessions in order. */
const world = (setup: Setup, condition: Condition, n: number, truths: Map<string, Truth>) =>
  Effect.scoped(
    Effect.gen(function* () {
      const ws = yield* makeWorkspace({ memory: false });
      yield* warmUp(ws, setup, MODEL);
      const sessions: Session[] = [];
      for (const [i, org] of ORGS.entries()) {
        const ask = condition === "forge" && i === 0 ? forgeAsk[setup] : "";
        const prompt = `${job(org)}${ask}${ANSWER_RULE}`;
        const outcome = yield* Effect.result(
          runClaude(yield* argsFor(setup, prompt, ws, crypto.randomUUID().slice(0, 8), MODEL), {
            cwd: workDir(ws, setup),
            env: envFor(setup, ws),
            timeout: RUN_TIMEOUT,
          }),
        );
        const base = { setup, condition, world: n, session: i + 1, org };
        const record: Session = Result.isSuccess(outcome)
          ? { ...base, correct: correct(outcome.success.answer, truths.get(org)!), usedForged: usedForged(outcome.success), metrics: outcome.success }
          : { ...base, correct: false, usedForged: false, error: `${outcome.failure._tag}` };
        yield* Console.log(
          `  ${record.correct ? "right" : "WRONG"}  ${setup.padEnd(8)} ${condition.padEnd(8)} w${n} s${i + 1} ${org.padEnd(20)} ${
            record.metrics ? `$${record.metrics.costUsd.toFixed(4)} ${record.metrics.seconds.toFixed(1)}s ${record.metrics.turns}t${record.usedForged ? " used-forged" : ""}` : record.error
          }`,
        );
        sessions.push(record);
      }
      const forged = yield* Effect.promise(() => Array.fromAsync(Deno.readDir(`${ws.root}/forged`)).catch(() => []));
      yield* Console.log(`  world ${setup}/${condition}/${n}: forged files ${forged.map((f) => f.name).join(", ") || "none"}`);
      return sessions;
    }),
  );

const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);
const median = (xs: number[]) => {
  if (!xs.length) return 0;
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

const report = (all: ReadonlyArray<Session>) => {
  const rows = SETUPS.flatMap((setup) =>
    CONDITIONS.map((condition) => {
      const of = (pred: (s: Session) => boolean) => all.filter((s) => s.setup === setup && s.condition === condition && pred(s));
      const later = of((s) => s.session > 1);
      const first = of((s) => s.session === 1);
      const m = (xs: Session[]) => xs.flatMap((s) => (s.metrics ? [s.metrics] : []));
      return {
        setup,
        condition,
        line: `| ${setup} | ${condition} | ${later.filter((s) => s.correct).length}/${later.length} | $${mean(m(later).map((x) => x.costUsd)).toFixed(4)} | ${
          median(m(later).map((x) => x.seconds)).toFixed(1)
        }s | ${median(m(later).map((x) => x.turns))} | ${condition === "forge" ? `${later.filter((s) => s.usedForged).length}/${later.length}` : "-"} | $${
          mean(m(first).map((x) => x.costUsd)).toFixed(4)
        } |`,
        laterCost: mean(m(later).map((x) => x.costUsd)),
        firstCost: mean(m(first).map((x) => x.costUsd)),
      };
    })
  );
  const breakEven = SETUPS.map((setup) => {
    const f = rows.find((r) => r.setup === setup && r.condition === "forge")!;
    const n = rows.find((r) => r.setup === setup && r.condition === "no-forge")!;
    const saved = n.laterCost - f.laterCost;
    const extra = f.firstCost - n.firstCost;
    return `- ${setup}: forging costs $${extra.toFixed(4)} extra once and saves $${saved.toFixed(4)} per later job${
      saved > 0 ? `, so it pays for itself after ${Math.max(1, Math.ceil(extra / saved))} reuse(s).` : ", so it does not pay off here."
    }`;
  });
  return [
    "| Setup | Condition | Sessions 2-5 right | Cost per job, sessions 2-5 (mean) | Time (median) | Turns (median) | Used the forged tool | Session 1 cost (mean) |",
    "|---|---|---|---|---|---|---|---|",
    ...rows.map((r) => r.line),
    "",
    ...breakEven,
  ].join("\n");
};

const program = Effect.gen(function* () {
  const truths = new Map<string, Truth>();
  for (const org of ORGS) truths.set(org, yield* truthOf(org));
  yield* Console.log(`Answers: ${JSON.stringify(Object.fromEntries(truths))}`);
  const plan = SETUPS.flatMap((setup) => CONDITIONS.flatMap((condition) => Array.from({ length: WORLDS }, (_, i) => ({ setup, condition, n: i + 1 }))));
  yield* Console.log(`Running ${plan.length} worlds of ${ORGS.length} sessions, ${CONCURRENCY} at a time, model ${MODEL}`);
  const all = (yield* Effect.forEach(plan, (p) => world(p.setup, p.condition, p.n, truths), { concurrency: CONCURRENCY })).flat();
  const date = new Date().toISOString().slice(0, 10);
  const commit = (yield* runCommand("git", ["-C", HERE, "rev-parse", "--short", "HEAD"])).trim();
  const dogu = (yield* runCommand("dogu", ["--version"])).trim();
  const text = [
    `# Forge benchmark ${date}`,
    "",
    `${dogu}, model \`${MODEL}\`, ${WORLDS} worlds per setup and condition, bench at commit ${commit}. Job: the latest release of each of an org's 3 most-starred repos; orgs in order: ${ORGS.join(", ")}.`,
    "",
    report(all),
    "",
  ].join("\n");
  const out = `${HERE}results/${env("BENCH_OUT", `${date}-forge`)}`;
  yield* Effect.promise(() => Deno.writeTextFile(`${out}.md`, text));
  yield* Effect.promise(() => Deno.writeTextFile(`${out}.json`, JSON.stringify({ date, dogu, model: MODEL, commit, truths: Object.fromEntries(truths), sessions: all }, null, 2)));
  yield* Console.log(`\n${text}\nWrote ${out}.md and .json`);
});

Effect.runPromise(program).catch((e) => {
  console.error("forge benchmark failed:", e);
  Deno.exit(1);
});
