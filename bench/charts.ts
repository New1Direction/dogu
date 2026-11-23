// Draws the charts in BENCHMARKS.md from the results files, so a chart cannot disagree with its data.
//
//   deno run --allow-read --allow-write bench/charts.ts
//
// Colors and helpers are shared with the pictures, in svg.ts.
// Every chart also has its numbers in a table next to it in BENCHMARKS.md.

import { change, cost, esc, load, mean, type Rec, svg as frame, time, tokens, write } from "./svg.ts";

// --- Drawing ---

const W = 720;
const LABEL = 290; // the label column
const PLOT_L = LABEL + 10;
const PLOT_R = W - 70; // room for value labels
const ROW = 34;
const BAR = 18;
const TOP = 44; // room for the legend

/** A bar with a 4px rounded data end and a square end on the baseline. */
const bar = (x0: number, x1: number, y: number, cls: string) => {
  const [left, right] = x0 <= x1 ? [x0, x1] : [x1, x0];
  const w = Math.max(right - left, 1);
  const r = Math.min(4, w);
  const roundRight = x1 >= x0;
  const path = roundRight
    ? `M${left},${y} h${w - r} a${r},${r} 0 0 1 ${r},${r} v${BAR - 2 * r} a${r},${r} 0 0 1 ${-r},${r} h${-(w - r)} z`
    : `M${right},${y} h${-(w - r)} a${r},${r} 0 0 0 ${-r},${r} v${BAR - 2 * r} a${r},${r} 0 0 0 ${r},${r} h${w - r} z`;
  return `<path class="${cls}" d="${path}"/>`;
};

const legend = (items: ReadonlyArray<[string, string]>) =>
  items.map(([cls, text], i) => {
    const x = PLOT_L + i * 190;
    return `<rect class="${cls}" x="${x}" y="12" width="12" height="12" rx="3"/><text x="${x + 18}" y="22">${esc(text)}</text>`;
  }).join("");

interface Row {
  readonly label: string;
  readonly value: number;
  readonly cls: string;
  readonly shown: string;
}

/** Horizontal bars from zero. */
const bars = (title: string, rows: ReadonlyArray<Row>, max: number, ticks: ReadonlyArray<[number, string]>, items: ReadonlyArray<[string, string]>) => {
  const h = TOP + rows.length * ROW + 30;
  const x = (v: number) => PLOT_L + (v / max) * (PLOT_R - PLOT_L);
  const grid = ticks.map(([v, t]) =>
    `<line class="grid" x1="${x(v)}" x2="${x(v)}" y1="${TOP - 4}" y2="${h - 26}"/><text class="muted" x="${x(v)}" y="${h - 10}" text-anchor="middle">${t}</text>`
  ).join("");
  const body = rows.map((r, i) => {
    const y = TOP + i * ROW + (ROW - BAR) / 2;
    return `<text x="${LABEL}" y="${y + 13}" text-anchor="end">${esc(r.label)}</text>${bar(PLOT_L, x(r.value), y, r.cls)}<text x="${x(r.value) + 6}" y="${y + 13}">${esc(r.shown)}</text>`;
  }).join("");
  return svg(title, h, `${legend(items)}${grid}<line class="axis" x1="${PLOT_L}" x2="${PLOT_L}" y1="${TOP - 4}" y2="${h - 26}"/>${body}`);
};

/** Bars either side of zero: left is cheaper, right costs more. */
const COST_KEY: ReadonlyArray<[string, string]> = [["dogu", "Cheaper with dogu"], ["more", "Costs more with dogu"]];

const diverging = (
  title: string,
  rows: ReadonlyArray<Row>,
  min: number,
  max: number,
  ticks: ReadonlyArray<[number, string]>,
  key: ReadonlyArray<[string, string]> = COST_KEY,
) => {
  const h = TOP + rows.length * ROW + 30;
  const x = (v: number) => PLOT_L + ((v - min) / (max - min)) * (PLOT_R - PLOT_L);
  const grid = ticks.map(([v, t]) =>
    `<line class="${v === 0 ? "axis" : "grid"}" x1="${x(v)}" x2="${x(v)}" y1="${TOP - 4}" y2="${h - 26}"/><text class="muted" x="${x(v)}" y="${h - 10}" text-anchor="middle">${t}</text>`
  ).join("");
  const body = rows.map((r, i) => {
    const y = TOP + i * ROW + (ROW - BAR) / 2;
    const end = x(r.value);
    const labelX = r.value < 0 ? end - 6 : end + 6;
    return `<text x="${LABEL}" y="${y + 13}" text-anchor="end">${esc(r.label)}</text>${bar(x(0), end, y, r.cls)}<text x="${labelX}" y="${y + 13}" text-anchor="${r.value < 0 ? "end" : "start"}">${esc(r.shown)}</text>`;
  }).join("");
  return svg(title, h, `${legend(key)}${grid}${body}`);
};

const svg = (title: string, h: number, body: string) => frame(title, W, h, body);

// --- The charts ---

// The headline runs: Claude Code with Sonnet, dogu 0.7.0 (7 of the 8 held-out tasks; Context7's free
// quota was used up, so its task is left out). The 0.5.0 runs are in 2026-11-22-heldout-sonnet.
const sonnet = load("2026-11-22-heldout-v0.7.0");
const haiku = load("2026-11-22-heldout-haiku-v0.7.0");
const pi = load("2026-11-22-heldout-pi");
// Codex with ~/.npm writable in its sandbox, as dogu's README says (the earlier run without it is in
// 2026-11-22-heldout-codex), at normal speed and in fast mode.
const codex = load("2026-11-22-heldout-codex-fast0");
const codexFast = load("2026-11-22-heldout-codex-fast1");
const terra = load("2026-11-22-heldout-codex-terra-fast0");
const terraFast = load("2026-11-22-heldout-codex-terra-fast1");
const forge = load("2026-11-22-forge");
const pi62 = load("2026-11-22-pi-gpt62");
const codexCodemode = load("2026-11-22-codex-codemode");
// MCPMark's 40 filesystem tasks (a public benchmark, its own checkers), dogu 0.7.5.
const mcpmark = load("2026-11-22-mcpmark-filesystem");

const claudeSetups: ReadonlyArray<[string, string, string]> = [
  ["direct", "Loading the servers directly", "other"],
  ["direct-search", "Directly, with Claude Code's tool search", "other"],
  ["folder", "dogu folder", "dogu"],
  ["codemode", "dogu code mode", "dogu"],
];
const KEY: ReadonlyArray<[string, string]> = [["other", "Without dogu"], ["dogu", "With dogu"]];

const charts: Record<string, string> = {
  "cost.svg": bars(
    "Cost per task, Claude Code with Sonnet, dogu 0.7.0, 7 held-out tasks",
    claudeSetups.map(([s, label, cls]) => ({ label, value: cost(sonnet, s), cls, shown: `$${cost(sonnet, s).toFixed(3)}` })),
    0.04,
    [[0, "$0"], [0.01, "$0.01"], [0.02, "$0.02"], [0.03, "$0.03"], [0.04, "$0.04"]],
    KEY,
  ),
  "time.svg": bars(
    "Time per task, Claude Code with Sonnet, dogu 0.7.0, 7 held-out tasks",
    claudeSetups.map(([s, label, cls]) => ({ label, value: time(sonnet, s), cls, shown: `${time(sonnet, s).toFixed(1)} s` })),
    6,
    [[0, "0 s"], [2, "2 s"], [4, "4 s"], [6, "6 s"]],
    KEY,
  ),
  "agents.svg": (() => {
    const rows: Row[] = [
      ["Claude Code, Sonnet: vs loading directly", change(cost(sonnet, "folder"), cost(sonnet, "direct"))],
      ["Claude Code, Sonnet: vs its tool search", change(cost(sonnet, "folder"), cost(sonnet, "direct-search"))],
      ["Claude Code, Haiku: vs loading directly", change(cost(haiku, "folder"), cost(haiku, "direct"))],
      ["Claude Code, Haiku: vs its tool search", change(cost(haiku, "folder"), cost(haiku, "direct-search"))],
      ["pi 6.2, code mode: vs loading directly", change(cost(pi62, "pi-dogu"), cost(pi62, "pi-direct"))],
      ["pi 6.2, code mode: vs pi's code mode", change(cost(pi62, "pi-dogu"), cost(pi62, "pi-codemode"))],
      ["pi 6.1, folder: vs loading directly", change(cost(pi, "pi-folder"), cost(pi, "pi-direct"))],
      ["pi 6.1, folder: vs pi's code mode", change(cost(pi, "pi-folder"), cost(pi, "pi-codemode"))],
      ["Codex, folder: vs loading directly (tokens)", change(tokens(codex, "codex-folder"), tokens(codex, "codex-direct"))],
      ["Codex, folder: vs Codex's code mode (tokens)", change(tokens(codexCodemode, "codex-folder"), tokens(codexCodemode, "codex-codemode"))],
    ].map(([label, v]) => ({
      label: label as string,
      value: v as number,
      cls: (v as number) <= 0 ? "dogu" : "more",
      shown: `${(v as number) > 0 ? "+" : ""}${v}%`,
    }));
    return diverging("Change in cost per task with dogu, by agent", rows, -100, 40, [[-80, "-80%"], [-40, "-40%"], [0, "0"], [40, "+40%"]]);
  })(),
  "mcpmark.svg": (() => {
    const m = (setup: string, f: (x: NonNullable<Rec["metrics"]>) => number, agg: "mean" | "median") => {
      const xs = mcpmark.filter((r) => r.setup === setup && r.metrics).map((r) => f(r.metrics!));
      return agg === "mean" ? mean(xs) : [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
    };
    type Pick = readonly [string, (x: NonNullable<Rec["metrics"]>) => number, "mean" | "median"];
    const picks: ReadonlyArray<Pick> = [
      ["Cost per task (mean)", (x) => x.costUsd ?? 0, "mean"],
      ["Cost per task (median)", (x) => x.costUsd ?? 0, "median"],
      ["Time per task (median)", (x) => x.seconds, "median"],
      ["Time per task (mean)", (x) => x.seconds, "mean"],
      ["Tokens read per task (median)", (x) => x.totalInputTokens, "median"],
      ["Model turns per task (mean)", (x) => x.turns, "mean"],
    ];
    const rows: Row[] = picks.map(([label, f, agg]) => {
      const v = change(m("codemode", f, agg), m("direct", f, agg));
      return { label, value: v, cls: v <= 0 ? "dogu" : "more", shown: `${v > 0 ? "+" : ""}${v}%` };
    });
    return diverging("MCPMark filesystem tasks: dogu code mode against loading the server directly", rows, -100, 20, [[-100, "-100%"], [-50, "-50%"], [0, "0"]], [["dogu", "Less with dogu"], ["more", "More with dogu"]]);
  })(),
  "fast.svg": bars(
    "Time per task with Codex, normal speed and fast mode, two models",
    [
      ["Default model, directly, normal", time(codex, "codex-direct"), "other"],
      ["Default model, directly, fast", time(codexFast, "codex-direct"), "other"],
      ["Default model, dogu, normal", time(codex, "codex-folder"), "dogu"],
      ["Default model, dogu, fast", time(codexFast, "codex-folder"), "dogu"],
      ["Terra, directly, normal", time(terra, "codex-direct"), "other"],
      ["Terra, directly, fast", time(terraFast, "codex-direct"), "other"],
      ["Terra, dogu, normal", time(terra, "codex-folder"), "dogu"],
      ["Terra, dogu, fast", time(terraFast, "codex-folder"), "dogu"],
    ].map(([label, v, cls]) => ({ label: label as string, value: v as number, cls: cls as string, shown: `${(v as number).toFixed(1)} s` })),
    15,
    [[0, "0 s"], [5, "5 s"], [10, "10 s"], [15, "15 s"]],
    KEY,
  ),
  "forge.svg": (() => {
    const steady = (setup: string, condition: string) =>
      mean(forge.filter((r) => r.setup === setup && r.condition === condition && (r.session ?? 0) >= 3 && r.metrics).map((r) => r.metrics!.costUsd ?? 0));
    const rows: Row[] = [
      ["dogu folder, no forged tool", steady("folder", "no-forge"), "other"],
      ["dogu folder, with the forged tool", steady("folder", "forge"), "dogu"],
      ["dogu code mode, no forged tool", steady("codemode", "no-forge"), "other"],
      ["dogu code mode, with the forged tool", steady("codemode", "forge"), "dogu"],
    ].map(([label, v, cls]) => ({ label: label as string, value: v as number, cls: cls as string, shown: `$${(v as number).toFixed(4)}` }));
    return bars("Cost per repeated job, Sonnet", rows, 0.02, [[0, "$0"], [0.005, "$0.005"], [0.01, "$0.01"], [0.015, "$0.015"], [0.02, "$0.02"]], [["other", "No forged tool"], ["dogu", "With the forged tool"]]);
  })(),
};

write(charts);
