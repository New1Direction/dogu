// Turns the run records into the summary tables.

import type { RunMetrics } from "./claude.ts";

export interface RunRecord {
  readonly setup: string;
  readonly task: string;
  readonly suite: string;
  readonly rep: number;
  readonly correct: boolean;
  readonly metrics?: RunMetrics;
  readonly error?: string;
}

const mean = (xs: ReadonlyArray<number>) => (xs.length === 0 ? 0 : xs.reduce((a, b) => a + b, 0) / xs.length);
const median = (xs: ReadonlyArray<number>) => {
  if (xs.length === 0) return 0;
  const s = [...xs].sort((a, b) => a - b);
  const mid = Math.floor(s.length / 2);
  return s.length % 2 === 1 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
};
const k = (n: number) => `${(n / 1000).toFixed(1)}k`;
const pct = (part: number, whole: number) => (whole === 0 ? "0%" : `${Math.round((part / whole) * 100)}%`);

export const SETUP_LABELS: Record<string, string> = {
  direct: "MCP servers loaded directly",
  "direct-search": "Loaded directly, Claude Code tool search on",
  folder: "dogu folder, list in instructions",
  "folder-read": "dogu folder, \"read the README\"",
  codemode: "dogu serve (code mode)",
};

/** One row per setup: how often it was right, and what it cost on average. */
export const summaryTable = (records: ReadonlyArray<RunRecord>, setups: ReadonlyArray<string>): string => {
  const rows = setups.map((setup) => {
    const all = records.filter((r) => r.setup === setup);
    const done = all.flatMap((r) => (r.metrics ? [r.metrics] : []));
    return `| ${SETUP_LABELS[setup] ?? setup} | ${all.filter((r) => r.correct).length}/${all.length} | ${
      k(median(done.map((m) => m.firstTurnTokens)))
    } | ${k(median(done.map((m) => m.totalInputTokens)))} | $${mean(done.map((m) => m.costUsd)).toFixed(3)} | ${
      median(done.map((m) => m.seconds)).toFixed(1)
    }s | ${median(done.map((m) => m.turns))} | ${done.filter((m) => m.bypassedTools).length} |`;
  });
  return [
    "| Setup | Correct | First-turn tokens (median) | Total input tokens (median) | Cost per task (mean) | Time (median) | Turns (median) | Went around the tools |",
    "|---|---|---|---|---|---|---|---|",
    ...rows,
  ].join("\n");
};

/** One row per task: median total input tokens for each setup, and how the folder compares to direct. */
export const taskTable = (
  records: ReadonlyArray<RunRecord>,
  setups: ReadonlyArray<string>,
  tasks: ReadonlyArray<{ readonly id: string; readonly kind: string }>,
): string => {
  const cell = (setup: string, task: string) => {
    const rs = records.filter((r) => r.setup === setup && r.task === task);
    const tokens = median(rs.flatMap((r) => (r.metrics ? [r.metrics.totalInputTokens] : [])));
    return { tokens, text: `${k(tokens)} (${rs.filter((r) => r.correct).length}/${rs.length} right)` };
  };
  const header = `| Task | ${setups.map((s) => SETUP_LABELS[s] ?? s).join(" | ")} |`;
  const rows = tasks.map((t) => `| ${t.id} (${t.kind}) | ${setups.map((s) => cell(s, t.id).text).join(" | ")} |`);
  return [header, `|---|${setups.map(() => "---").join("|")}|`, ...rows].join("\n");
};

/** "36% fewer" or "12% more", from a ratio against the baseline. */
const change = (ratio: number, less: string, more: string) =>
  ratio <= 1 ? `${pct(1 - ratio, 1)} ${less}` : `${pct(ratio - 1, 1)} ${more}`;

/** Headline numbers for each dogu setup, against loading the servers directly with and without tool search. */
export const headline = (records: ReadonlyArray<RunRecord>, setups: ReadonlyArray<string>): string => {
  const metricsOf = (setup: string) => records.filter((r) => r.setup === setup && r.metrics).map((r) => r.metrics!);
  const versus = (setup: string, baseline: string) => {
    const base = metricsOf(baseline);
    const ms = metricsOf(setup);
    if (base.length === 0 || ms.length === 0) return "";
    const tokens = median(ms.map((m) => m.totalInputTokens)) / median(base.map((m) => m.totalInputTokens));
    const cost = mean(ms.map((m) => m.costUsd)) / mean(base.map((m) => m.costUsd));
    const time = median(ms.map((m) => m.seconds)) / median(base.map((m) => m.seconds));
    return `${change(tokens, "fewer", "more")} input tokens, ${change(cost, "lower", "higher")} cost and ${change(time, "less", "more")} time than ${SETUP_LABELS[baseline].toLowerCase()}`;
  };
  return setups
    .filter((s) => !s.startsWith("direct"))
    .flatMap((s) =>
      ["direct", "direct-search"].filter((b) => setups.includes(b)).map((b) => versus(s, b)).filter((t) => t !== "")
        .map((t) => `- ${SETUP_LABELS[s]}: ${t}.`)
    )
    .join("\n");
};
