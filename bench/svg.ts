// What the charts (charts.ts) and the pictures (pictures.ts) share: reading results, averaging, colors.
//
// Colors follow a validated palette (the dataviz skill's validator, light and dark): blue for dogu,
// orange for without dogu, red for "costs more"; aqua, violet and yellow for the parts of a run in
// the time picture, chosen away from blue and orange, which already mean dogu and not-dogu here.

export const HERE = new URL(".", import.meta.url).pathname;
export const OUT = `${HERE}../docs/bench`;

export interface Segment {
  readonly kind: "startup" | "model" | "tools";
  readonly seconds: number;
}
export interface Metrics {
  readonly costUsd: number | null;
  readonly seconds: number;
  readonly totalInputTokens: number;
  readonly firstTurnTokens: number | null;
  readonly timeline?: ReadonlyArray<Segment>;
}
export interface Rec {
  readonly setup: string;
  readonly suite?: string;
  readonly correct?: boolean;
  readonly metrics?: Metrics;
  readonly session?: number;
  readonly condition?: string;
}

export const load = (name: string): Rec[] => {
  const json = JSON.parse(Deno.readTextFileSync(`${HERE}results/${name}.json`));
  return (json.records ?? json.sessions) as Rec[];
};
export const ofSetup = (rs: Rec[], setup: string) => rs.filter((r) => r.setup === setup && r.metrics).map((r) => r.metrics!);
export const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;
export const median = (xs: number[]) => {
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
export const cost = (rs: Rec[], setup: string) => mean(ofSetup(rs, setup).map((m) => m.costUsd ?? 0));
export const time = (rs: Rec[], setup: string) => median(ofSetup(rs, setup).map((m) => m.seconds));
export const tokens = (rs: Rec[], setup: string) => median(ofSetup(rs, setup).map((m) => m.totalInputTokens));
export const firstTurn = (rs: Rec[], setup: string) => median(ofSetup(rs, setup).map((m) => m.firstTurnTokens ?? 0));
export const change = (ours: number, theirs: number) => Math.round((ours / theirs - 1) * 100);

export const STYLE = `
  text { font-family: system-ui, -apple-system, "Segoe UI", sans-serif; font-size: 13px; fill: #0b0b0b; }
  .muted { fill: #52514e; font-size: 12px; }
  .big { font-size: 34px; font-weight: 650; }
  .head { font-size: 15px; font-weight: 600; }
  .grid { stroke: #e1e0d9; stroke-width: 1; }
  .axis { stroke: #c3c2b7; stroke-width: 1; }
  .line { stroke: #52514e; stroke-width: 1.5; fill: none; }
  .card { fill: #f3f2ee; stroke: #e1e0d9; }
  .page { fill: #fcfcfb; stroke: #8a8983; stroke-width: 1; }
  .dogu { fill: #2a78d6; } .other { fill: #eb6834; } .more { fill: #e34948; }
  .s-dogu { stroke: #2a78d6; } .s-other { stroke: #eb6834; }
  .startup { fill: #eda100; } .model { fill: #6250d6; } .tools { fill: #1baf7a; }
  .gap { stroke: #fcfcfb; stroke-width: 2; }
  @media (prefers-color-scheme: dark) {
    text { fill: #f0efec; } .muted { fill: #c3c2b7; }
    .grid { stroke: #2c2c2a; } .axis { stroke: #383835; } .line { stroke: #c3c2b7; }
    .card { fill: #232322; stroke: #383835; } .page { fill: #1a1a19; stroke: #8a8983; }
    .dogu { fill: #3987e5; } .other { fill: #d95926; } .more { fill: #e66767; }
    .s-dogu { stroke: #3987e5; } .s-other { stroke: #d95926; }
    .startup { fill: #c98500; } .model { fill: #9085e9; } .tools { fill: #199e70; }
    .gap { stroke: #1a1a19; }
  }`;

export const esc = (s: string) => s.replaceAll("&", "&amp;").replaceAll("<", "&lt;");

/** The SVG around a picture: a title for screen readers, the shared style, a size from the viewBox. */
export const svg = (title: string, w: number, h: number, body: string) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" role="img" aria-labelledby="t">` +
  `<title id="t">${esc(title)}</title><style>${STYLE}</style>${body}</svg>\n`;

export const write = (pictures: Record<string, string>) => {
  Deno.mkdirSync(OUT, { recursive: true });
  for (const [name, text] of Object.entries(pictures)) {
    Deno.writeTextFileSync(`${OUT}/${name}`, text);
    console.log(`wrote docs/bench/${name}`);
  }
};
