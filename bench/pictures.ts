// Draws the explanatory pictures in BENCHMARKS.md from the results files: how each way works, the
// headline numbers, and where the seconds of a task go. Like the charts, every number is computed here
// from bench/results, never typed in.
//
//   deno run --allow-read --allow-write bench/pictures.ts

import { change, cost, esc, firstTurn, load, mean, type Rec, type Segment, svg, time, tokens, write } from "./svg.ts";

const sonnet = load("2026-11-22-heldout-sonnet");
const terra = load("2026-11-22-heldout-codex-terra-fast0");

// --- How each way works ---

/** Tokens drawn as one page, so a stack's height is what the agent reads before it starts. */
const TOKENS_PER_PAGE = 2_500;

interface Lane {
  readonly title: string;
  readonly cls: "other" | "dogu";
  readonly setup: string;
  readonly reads: string; // what is on the pages
  readonly sends: string; // what the agent sends to its tools
  readonly middle: string | null; // dogu's part, if any
  readonly together: boolean; // calls go out together
}

const LANES: ReadonlyArray<Lane> = [
  { title: "Loading the servers directly", cls: "other", setup: "direct", reads: "every tool's full manual", sends: "tool calls by name", middle: null, together: false },
  { title: "dogu folder", cls: "dogu", setup: "folder", reads: "one line per tool", sends: "a short command", middle: "dogu", together: false },
  { title: "dogu code mode", cls: "dogu", setup: "codemode", reads: "the tool names", sends: "one small program", middle: "dogu sandbox", together: true },
];

const LANE_W = 250;
const SERVERS = ["GitHub", "Context7", "DeepWiki"];

const arrow = (x1: number, y1: number, x2: number, y2: number) =>
  `<line class="line" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" marker-end="url(#head)"/>`;

const box = (x: number, y: number, w: number, h: number, label: string, cls = "card") =>
  `<rect class="${cls}" x="${x}" y="${y}" width="${w}" height="${h}" rx="6"/><text x="${x + w / 2}" y="${y + h / 2 + 4}" text-anchor="middle">${esc(label)}</text>`;

/** A stack of n pages, the front one with a few lines of text; the stack is centred on cx. */
const pages = (cx: number, bottom: number, n: number, cls: string) => {
  const w = 64, h = 84, step = 3;
  const frontX = cx - w / 2 - ((n - 1) * step) / 2;
  const frontY = bottom - h;
  const sheets = Array.from({ length: n }, (_, k) => n - 1 - k) // back to front
    .map((k) => `<rect class="page" x="${frontX + k * step}" y="${frontY - k * step}" width="${w}" height="${h}" rx="3"/>`);
  const lines = [16, 28, 40, 52, 64].map((dy, i) =>
    `<rect class="${cls}" x="${frontX + 10}" y="${frontY + dy}" width="${i % 2 ? 30 : 44}" height="4" rx="2"/>`
  );
  return sheets.join("") + lines.join("");
};

/** The agent: a rounded head with two eyes. */
const agent = (cx: number, y: number) =>
  `<rect class="card" x="${cx - 30}" y="${y}" width="60" height="42" rx="12"/>` +
  `<circle class="line" cx="${cx - 11}" cy="${y + 19}" r="4"/><circle class="line" cx="${cx + 11}" cy="${y + 19}" r="4"/>` +
  `<text class="muted" x="${cx + 38}" y="${y + 25}">agent</text>`;

const lane = (l: Lane, i: number) => {
  const x0 = 10 + i * LANE_W;
  const cx = x0 + LANE_W / 2;
  const read = firstTurn(sonnet, l.setup);
  const n = Math.max(1, Math.round(read / TOKENS_PER_PAGE));
  const parts: Array<string> = [
    `<rect class="${l.cls}" x="${x0 + 14}" y="18" width="6" height="18" rx="2"/>`,
    `<text class="head" x="${x0 + 28}" y="32">${esc(l.title)}</text>`,
    `<text class="muted" x="${cx}" y="62" text-anchor="middle">reads before every task</text>`,
    pages(cx, 196, n, l.cls),
    `<text x="${cx}" y="222" text-anchor="middle" font-weight="600">${Math.round(read / 1000)}k tokens</text>`,
    `<text class="muted" x="${cx}" y="240" text-anchor="middle">${esc(l.reads)}</text>`,
    agent(cx, 262),
  ];
  const serversY = 430;
  const sw = 70;
  const sx = (j: number) => x0 + 8 + j * (sw + 9);
  parts.push(...SERVERS.map((s, j) => box(sx(j), serversY, sw, 30, s)));
  if (l.middle === null) {
    parts.push(arrow(cx, 304, cx, serversY - 4));
    parts.push(`<text class="muted" x="${cx + 8}" y="370">${esc(l.sends)}</text>`);
  } else {
    parts.push(arrow(cx, 304, cx, 348));
    parts.push(`<text class="muted" x="${cx + 8}" y="330">${esc(l.sends)}</text>`);
    parts.push(box(cx - 60, 352, 120, 30, l.middle));
    const targets = l.together ? [0, 1, 2] : [1];
    parts.push(...targets.map((j) => arrow(cx, 382, sx(j) + sw / 2, serversY - 4)));
    if (l.together) parts.push(`<text class="muted" x="${cx}" y="${serversY + 50}" text-anchor="middle">calls run at the same time;</text>`);
    if (l.together) parts.push(`<text class="muted" x="${cx}" y="${serversY + 66}" text-anchor="middle">only what it prints comes back</text>`);
  }
  return parts.join("");
};

const howItWorks = () => {
  const w = LANE_W * 3 + 20;
  const h = 510;
  const defs = `<defs><marker id="head" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">` +
    `<path class="line" d="M0,0 L10,5 L0,10" fill="none"/></marker></defs>`;
  const dividers = [1, 2].map((i) => `<line class="grid" x1="${10 + i * LANE_W}" x2="${10 + i * LANE_W}" y1="16" y2="${h - 16}"/>`).join("");
  return svg(
    "How each way gives an agent its tools: what it reads before every task, and what it sends",
    w,
    h,
    defs + dividers + LANES.map(lane).join(""),
  );
};

// --- The headline numbers ---

interface Tile {
  readonly big: string;
  readonly line1: string;
  readonly line2: string;
  readonly cls: "dogu" | "more";
}

/** Right answers on the held-out tasks only, across every way tested. */
const correctOf = (rs: Rec[]) => {
  const heldOut = rs.filter((r) => r.suite === "heldout");
  return [heldOut.filter((r) => r.correct).length, heldOut.length];
};

const glance = () => {
  const [right, all] = correctOf(sonnet);
  const tiles: ReadonlyArray<Tile> = [
    { big: `${-change(cost(sonnet, "folder"), cost(sonnet, "direct"))}%`, line1: "lower cost per task", line2: "Claude Code, dogu folder", cls: "dogu" },
    { big: `${right} / ${all}`, line1: "answers right, every way", line2: "8 held-out tasks, Sonnet", cls: "dogu" },
    { big: `${-change(tokens(terra, "codex-folder"), tokens(terra, "codex-direct"))}%`, line1: "fewer tokens read", line2: "Codex with Terra", cls: "dogu" },
    {
      big: `+${(time(sonnet, "folder") - time(sonnet, "direct")).toFixed(1)} s`,
      line1: "slower on quick tasks",
      line2: "vs loading the tools directly",
      cls: "more",
    },
  ];
  // Two by two, so the tiles stay readable when GitHub shrinks the picture on a phone.
  const tw = 240, th = 108, gap = 12, cols = 2;
  const body = tiles.map((t, i) => {
    const x = 10 + (i % cols) * (tw + gap);
    const y = 10 + Math.floor(i / cols) * (th + gap);
    return `<rect class="card" x="${x}" y="${y}" width="${tw}" height="${th}" rx="10"/>` +
      `<rect class="${t.cls}" x="${x}" y="${y}" width="6" height="${th}" rx="3"/>` +
      `<text class="big" x="${x + 20}" y="${y + 48}">${esc(t.big)}</text>` +
      `<text x="${x + 20}" y="${y + 74}">${esc(t.line1)}</text>` +
      `<text class="muted" x="${x + 20}" y="${y + 92}">${esc(t.line2)}</text>`;
  }).join("");
  const rows = Math.ceil(tiles.length / cols);
  return svg("dogu at a glance: the headline numbers, including where it loses", 10 + cols * (tw + gap), 10 + rows * (th + gap), body);
};

// --- Where the seconds go ---

const KINDS: ReadonlyArray<[Segment["kind"], string]> = [
  ["startup", "Starting up (connecting to the tools)"],
  ["model", "The model working"],
  ["tools", "Tools running"],
];

/** Mean seconds per run in each part, from the timelines the bench records. */
const partsOf = (rs: Rec[], setup: string) => {
  const timed = rs.filter((r) => r.setup === setup && r.metrics?.timeline).map((r) => r.metrics!.timeline!);
  return KINDS.map(([kind]) => mean(timed.map((t) => t.filter((s) => s.kind === kind).reduce((n, s) => n + s.seconds, 0))));
};

const seconds = (file: string, rows: ReadonlyArray<[string, string]>) => {
  const rs = load(file);
  const W = 760, LABEL = 230, L = LABEL + 12, R = W - 60, ROW = 40, BAR = 22, TOP = 70;
  const parts = rows.map(([setup]) => partsOf(rs, setup));
  const max = Math.ceil(Math.max(...parts.map((p) => p.reduce((a, b) => a + b, 0))));
  const x = (v: number) => L + (v / max) * (R - L);
  const h = TOP + rows.length * ROW + 34;
  // Laid out one after another by an estimate of each label's width (about 7 px a letter at 13 px).
  const starts = KINDS.reduce<number[]>((xs, [, text], i) => [...xs, i === 0 ? 10 : xs[i - 1] + 18 + KINDS[i - 1][1].length * 7 + 26], []);
  const legend = KINDS.map(([kind, text], i) =>
    `<rect class="${kind}" x="${starts[i]}" y="14" width="12" height="12" rx="3"/><text x="${starts[i] + 18}" y="24">${esc(text)}</text>`
  ).join("");
  const ticks = Array.from({ length: max + 1 }, (_, v) => v).map((v) =>
    `<line class="grid" x1="${x(v)}" x2="${x(v)}" y1="${TOP - 6}" y2="${h - 30}"/><text class="muted" x="${x(v)}" y="${h - 12}" text-anchor="middle">${v} s</text>`
  ).join("");
  const body = rows.map(([, label], i) => {
    const y = TOP + i * ROW + (ROW - BAR) / 2;
    let at = 0;
    const segs = parts[i].map((v, k) => {
      const x0 = x(at), x1 = x(at + v);
      at += v;
      // White on violet and aqua; dark on yellow, as the palette asks.
      const ink = KINDS[k][0] === "startup" ? "#0b0b0b" : "#ffffff";
      const inside = x1 - x0 > 44
        ? `<text x="${(x0 + x1) / 2}" y="${y + 15}" text-anchor="middle" style="fill:${ink}">${v.toFixed(1)}</text>`
        : "";
      return `<rect class="${KINDS[k][0]}" x="${x0}" y="${y}" width="${Math.max(x1 - x0, 0.5)}" height="${BAR}" rx="2"/>` +
        `<line class="gap" x1="${x1}" x2="${x1}" y1="${y}" y2="${y + BAR}"/>${inside}`;
    }).join("");
    return `<text x="${LABEL}" y="${y + 15}" text-anchor="end">${esc(label)}</text>${segs}<text x="${x(at) + 6}" y="${y + 15}">${at.toFixed(1)} s</text>`;
  }).join("");
  return svg("Where the seconds of a task go, from when each step happened", W, h, legend + ticks + body);
};

console.log(`pages: ${LANES.map((l) => `${l.setup} ${Math.round(firstTurn(sonnet, l.setup))} tokens`).join(", ")}`);
write({
  "how-it-works.svg": howItWorks(),
  "glance.svg": glance(),
  // The fair-order run (setups take turns going first), without the Context7 task (its quota ran out).
  "seconds.svg": seconds("2026-11-22-exp5-warm-rotated", [
    ["direct", "Loading the servers directly"],
    ["codemode", "dogu code mode, 0.5.0"],
    ["codemode-b", "dogu code mode, next version"],
  ]),
});
