// Turns the run records into the summary tables.

import type { RunMetrics } from "./claude.ts";

export interface RunRecord {
  readonly setup: string;
  readonly task: string;
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

/** Headline numbers versus the direct setup, for the README. */
export const headline = (records: ReadonlyArray<RunRecord>, setups: ReadonlyArray<string>): string => {
  const base = records.filter((r) => r.setup === "direct" && r.metrics).map((r) => r.metrics!);
  if (base.length === 0) return "";
  return setups
    .filter((s) => s !== "direct")
    .map((s) => {
      const ms = records.filter((r) => r.setup === s && r.metrics).map((r) => r.metrics!);
      const tokens = median(ms.map((m) => m.totalInputTokens)) / median(base.map((m) => m.totalInputTokens));
      const cost = mean(ms.map((m) => m.costUsd)) / mean(base.map((m) => m.costUsd));
      return `- ${SETUP_LABELS[s]}: ${change(tokens, "fewer", "more")} input tokens and ${change(cost, "lower", "higher")} cost than loading the servers directly.`;
    })
    .join("\n");
};
