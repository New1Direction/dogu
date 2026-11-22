// The benchmark tasks. Each fetches its own right answer live (or makes it up at random) before any
// agent runs, and checks the agent's answer against it.
//
// Two suites:
//   tuning   the 5 tasks dogu 0.3.0 was tuned against while watching the agents. Reported, but not proof.
//   heldout  written and committed before any run, and never used for tuning. This is the real test.

import { Effect } from "npm:effect@4.1.5";
import type { CommandFailed } from "./claude.ts";

/** What a task needs to fetch its answer: the GitHub API, a dogu tool, and the run's seeded facts. */
export interface Ctx {
  readonly gh: (path: string) => Effect.Effect<any, CommandFailed>;
  readonly tool: (path: string, args: ReadonlyArray<string>) => Effect.Effect<string, CommandFailed>;
  /** A word hidden in the memory server's graph, and the entity that holds it. Random each run. */
  readonly seeded: { readonly word: string; readonly entity: string };
}

export type Suite = "tuning" | "heldout";

/** The facts a right answer must contain. What "contain" means is up to each task's check. */
export type Facts = ReadonlyArray<string>;

export interface Task {
  readonly id: string;
  readonly suite: Suite;
  readonly kind: string;
  readonly prompt: string;
  readonly fetch: (ctx: Ctx) => Effect.Effect<Facts, CommandFailed>;
  readonly check: (answer: string, facts: Facts) => boolean;
}

const lower = (s: string) => s.toLowerCase();
const has = (text: string, part: string) => lower(text).includes(lower(part));
const hasAll = (text: string, parts: Facts) => parts.length > 0 && parts.every((p) => has(text, p));
const hasAny = (text: string, parts: Facts) => parts.some((p) => has(text, p));
/** The number as a whole word, so 12 does not match 120. */
const hasNumber = (text: string, n: string) => new RegExp(`(^|[^0-9])${n}([^0-9]|$)`).test(text);
const firstLine = (text: string) => text.trim().split("\n")[0] ?? "";

/** The longest word of a section title, which a paraphrase almost always keeps. */
const keyWord = (title: string) => title.split(/\W+/).reduce((best, w) => (w.length > best.length ? w : best), "");
const mostSections = (answer: string, sections: Facts) =>
  sections.length > 0 && sections.filter((s) => has(answer, keyWord(s))).length >= Math.ceil((sections.length * 2) / 3);

/** DeepWiki's structure as top-level section titles. */
const sectionsOf = (structure: string): Facts =>
  structure.split("\n").filter((l) => l.startsWith("- ")).map((l) => l.replace(/^- [\d.]+\s*/, ""));

const topRepos = (ctx: Ctx, org: string, n: number) =>
  ctx.gh(`search/repositories?q=org:${org}&sort=stars&per_page=${n}`).pipe(
    Effect.map((r) => (r.items as Array<{ name: string; full_name: string; open_issues_count: number }>)),
  );

const deepwikiSections = (ctx: Ctx, repo: string) =>
  ctx.tool("deepwiki/read_wiki_structure", ["--repoName", repo]).pipe(Effect.map(sectionsOf));

export const TASKS: ReadonlyArray<Task> = [
  // --- tuning ---
  {
    id: "lookup",
    suite: "tuning",
    kind: "one call",
    prompt: "What is the latest release tag of the GitHub repo modelcontextprotocol/typescript-sdk?",
    fetch: (ctx) => ctx.gh("repos/modelcontextprotocol/typescript-sdk/releases/latest").pipe(Effect.map((r) => [r.tag_name])),
    check: (a, f) => has(a, f[0].replace(/^v/, "")),
  },
  {
    id: "search",
    suite: "tuning",
    kind: "one call, filtered",
    prompt: "Which 3 repos in the modelcontextprotocol GitHub org have the most stars?",
    fetch: (ctx) => topRepos(ctx, "modelcontextprotocol", 3).pipe(Effect.map((rs) => rs.map((r) => r.name))),
    check: hasAll,
  },
  {
    id: "chain",
    suite: "tuning",
    kind: "two servers",
    prompt:
      "What are the top-level sections of the DeepWiki documentation for the most-starred repo in the modelcontextprotocol GitHub org?",
    fetch: (ctx) =>
      topRepos(ctx, "modelcontextprotocol", 1).pipe(Effect.flatMap((rs) => deepwikiSections(ctx, rs[0].full_name))),
    check: mostSections,
  },
  {
    id: "docs",
    suite: "tuning",
    kind: "Context7",
    prompt: "Using Context7, what is the Context7 library ID for Effect, the TypeScript library?",
    fetch: (ctx) =>
      ctx.tool("context7/resolve-library-id", ["--libraryName", "effect", "--query", "Effect TypeScript library"]).pipe(
        Effect.map((t) => [...t.matchAll(/library ID:\s*(\S+)/g)].map((m) => m[1])),
      ),
    check: hasAny,
  },
  {
    id: "compare",
    suite: "tuning",
    kind: "several calls",
    prompt:
      "Of the 3 most-starred repos in the modelcontextprotocol GitHub org, which one has the most open issues? Put just its name on the first line.",
    fetch: (ctx) =>
      Effect.gen(function* () {
        const top = yield* topRepos(ctx, "modelcontextprotocol", 3);
        const issuesOnly = yield* Effect.forEach(top, (r) =>
          ctx.gh(`search/issues?q=repo:${r.full_name}+is:issue+is:open&per_page=1`).pipe(
            Effect.map((s) => ({ name: r.name, count: s.total_count as number })),
          ));
        const most = (xs: ReadonlyArray<{ name: string; count: number }>) =>
          xs.reduce((a, b) => (b.count > a.count ? b : a)).name;
        // GitHub counts pull requests as issues; accept either way of counting.
        return [...new Set([most(top.map((r) => ({ name: r.name, count: r.open_issues_count }))), most(issuesOnly)])];
      }),
    check: (a, f) => hasAny(firstLine(a), f),
  },

  // --- held out: written before any run, never tuned against ---
  {
    id: "created",
    suite: "heldout",
    kind: "one call, exact",
    prompt: "On what date (YYYY-MM-DD) was the GitHub repo tokio-rs/axum created?",
    fetch: (ctx) => ctx.gh("repos/tokio-rs/axum").pipe(Effect.map((r) => [String(r.created_at).slice(0, 10)])),
    check: (a, f) => has(a, f[0]),
  },
  {
    id: "issue-title",
    suite: "heldout",
    kind: "one call, exact",
    prompt: "What is the title of issue #1 in the GitHub repo modelcontextprotocol/inspector?",
    fetch: (ctx) => ctx.gh("repos/modelcontextprotocol/inspector/issues/1").pipe(Effect.map((r) => [String(r.title)])),
    // Titles can hold quotes or backticks an agent drops; compare letters and digits only.
    check: (a, f) => lower(a).replace(/[^a-z0-9]/g, "").includes(lower(f[0]).replace(/[^a-z0-9]/g, "")),
  },
  {
    id: "release-tags",
    suite: "heldout",
    kind: "long: 6+ calls",
    prompt:
      "For each of the 5 most-starred repos in the modelcontextprotocol GitHub org, what is its latest release tag? Say 'none' for a repo with no releases.",
    fetch: (ctx) =>
      Effect.gen(function* () {
        const top = yield* topRepos(ctx, "modelcontextprotocol", 5);
        const tags = yield* Effect.forEach(top, (r) =>
          ctx.gh(`repos/${r.full_name}/releases/latest`).pipe(
            Effect.map((rel) => String(rel.tag_name)),
            Effect.orElseSucceed(() => ""),
          ));
        return tags.filter((t) => t !== "");
      }),
    check: (a, f) => f.every((tag) => has(a, tag.replace(/^v/, ""))),
  },
  {
    id: "docs-id",
    suite: "heldout",
    kind: "discovery, server not named",
    prompt: "Which library ID should I use to look up up-to-date documentation for the Next.js framework?",
    fetch: (ctx) =>
      ctx.tool("context7/resolve-library-id", ["--libraryName", "next.js", "--query", "Next.js framework documentation"]).pipe(
        Effect.map((t) => [...t.matchAll(/library ID:\s*(\S+)/g)].map((m) => m[1])),
      ),
    check: hasAny,
  },
  {
    id: "deno-sections",
    suite: "heldout",
    kind: "one call, long answer",
    prompt: "According to DeepWiki, what are the top-level sections of the documentation for the GitHub repo denoland/deno?",
    fetch: (ctx) => deepwikiSections(ctx, "denoland/deno"),
    check: mostSections,
  },
  {
    id: "memory-find",
    suite: "heldout",
    kind: "local server",
    prompt: "In the knowledge graph, which entity has an observation that mentions the word '{word}'? Give the entity's name.",
    fetch: (ctx) => Effect.succeed([ctx.seeded.entity]),
    check: (a, f) => has(a, f[0]),
  },
  {
    id: "tokio-sections",
    suite: "heldout",
    kind: "two servers, a number",
    prompt:
      "Find the most-starred repo in the tokio-rs GitHub org, then tell me how many top-level sections its DeepWiki documentation has.",
    fetch: (ctx) =>
      topRepos(ctx, "tokio-rs", 1).pipe(
        Effect.flatMap((rs) => deepwikiSections(ctx, rs[0].full_name)),
        Effect.map((sections) => [String(sections.length)]),
      ),
    check: (a, f) => hasNumber(a, f[0]),
  },
  {
    id: "axum-releases",
    suite: "heldout",
    kind: "big results, paging",
    prompt: "How many releases has the GitHub repo tokio-rs/axum published in total?",
    fetch: (ctx) =>
      ctx.gh("repos/tokio-rs/axum/releases?per_page=100&page=1").pipe(
        Effect.flatMap((first: unknown[]) =>
          first.length < 100
            ? Effect.succeed(first.length)
            : ctx.gh("repos/tokio-rs/axum/releases?per_page=100&page=2").pipe(
              Effect.map((second: unknown[]) => first.length + second.length),
            )
        ),
        Effect.map((n) => [String(n)]),
      ),
    check: (a, f) => hasNumber(a, f[0]),
  },
];

/** The prompt with this run's seeded facts filled in. */
export const promptOf = (task: Task, seeded: Ctx["seeded"]) => task.prompt.replaceAll("{word}", seeded.word);
