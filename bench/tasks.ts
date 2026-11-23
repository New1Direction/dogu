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
  readonly seeded: { readonly word: string; readonly entity: string; readonly year: number; readonly startedThen: string; readonly entityYear: number };
}

export type Suite = "tuning" | "heldout" | "heldout2" | "heldout3" | "heldout4" | "heldout5" | "heldout6" | "multistep" | "heldout8" | "heldout9" | "heldout10";

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
/** Every number written in the text, commas allowed: "92,161 stars" gives 92161. */
const numbersIn = (text: string) => [...text.matchAll(/\d[\d,]*(\.\d+)?/g)].map((m) => Number(m[0].replaceAll(",", "")));
/** Some number in the text within `tolerance` (a fraction) of n, for counts that move while a run goes. */
const near = (text: string, n: number, tolerance: number) =>
  numbersIn(text).some((x) => Math.abs(x - n) <= Math.max(1, n * tolerance));

const NUMBER_WORDS = [
  "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
  "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen", "twenty",
];

/** The number as a whole word, so 12 does not match 120; small numbers may be written out ("eight"). */
const hasNumber = (text: string, n: string) =>
  new RegExp(`(^|[^0-9])${n}([^0-9]|$)`).test(text) ||
  (NUMBER_WORDS[Number(n)] !== undefined && new RegExp(`\\b${NUMBER_WORDS[Number(n)]}\\b`, "i").test(text));
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

/** The latest release's tag of each repo, or "" for a repo with none (GitHub answers 404). */
const tagsOf = (ctx: Ctx, repos: ReadonlyArray<string>) =>
  Effect.forEach(repos, (r) =>
    ctx.gh(`repos/${r}/releases/latest`).pipe(
      Effect.map((rel) => ({ repo: r, tag: String(rel.tag_name), at: String(rel.published_at) })),
      Effect.orElseSucceed(() => ({ repo: r, tag: "", at: "" })),
    ));

/** Facts for "each repo's tag, and say which have none": the tags, a separator, then the repos without one. */
const tagsAndNones = (xs: ReadonlyArray<{ repo: string; tag: string }>) => [
  ...xs.filter((x) => x.tag !== "").map((x) => x.tag),
  "|",
  ...xs.filter((x) => x.tag === "").map((x) => x.repo.split("/")[1]),
];
const tagsAndNonesCheck = (a: string, f: Facts) => {
  const cut = f.indexOf("|");
  return hasAll(a, f.slice(0, cut)) && f.slice(cut + 1).every((n) => has(a, n));
};

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

  // --- held out, set 2: written and frozen before the code-mode speed work it is meant to judge ---
  {
    id: "open-prs",
    suite: "heldout2",
    kind: "a count",
    prompt: "How many open pull requests does the GitHub repo modelcontextprotocol/python-sdk have right now?",
    fetch: (ctx) =>
      ctx.gh("search/issues?q=repo:modelcontextprotocol/python-sdk+is:pr+is:open&per_page=1").pipe(Effect.map((r) => [String(r.total_count)])),
    // Pull requests open and close during a run; allow 2 either way.
    check: (a, f) => numbersIn(a).some((x) => Math.abs(x - Number(f[0])) <= 2),
  },
  {
    id: "oldest-issue",
    suite: "heldout2",
    kind: "sorting",
    prompt: "What is the title of the oldest open issue in the GitHub repo modelcontextprotocol/python-sdk?",
    fetch: (ctx) =>
      ctx.gh("search/issues?q=repo:modelcontextprotocol/python-sdk+is:issue+is:open&sort=created&order=asc&per_page=1").pipe(
        Effect.map((r) => [String(r.items[0].title)]),
      ),
    check: (a, f) => lower(a).replace(/[^a-z0-9]/g, "").includes(lower(f[0]).replace(/[^a-z0-9]/g, "")),
  },
  {
    id: "release-author",
    suite: "heldout2",
    kind: "a nested field",
    prompt: "Which GitHub user published the latest release of modelcontextprotocol/typescript-sdk? Give their login.",
    fetch: (ctx) => ctx.gh("repos/modelcontextprotocol/typescript-sdk/releases/latest").pipe(Effect.map((r) => [String(r.author.login)])),
    check: (a, f) => has(a, f[0]),
  },
  {
    id: "stars-sum",
    suite: "heldout2",
    kind: "adding up",
    prompt: "What is the total number of GitHub stars across the 5 most-starred repos in the denoland GitHub org?",
    fetch: (ctx) =>
      ctx.gh("search/repositories?q=org:denoland&sort=stars&per_page=5").pipe(
        Effect.map((r) => [String((r.items as Array<{ stargazers_count: number }>).reduce((s, i) => s + i.stargazers_count, 0))]),
      ),
    // Stars move a little during a run: within 1%.
    check: (a, f) => near(a, Number(f[0]), 0.01),
  },
  {
    id: "deepwiki-node",
    suite: "heldout2",
    kind: "finding one section",
    prompt: "In DeepWiki's documentation of the GitHub repo denoland/deno, which top-level section covers Node.js compatibility? Give its title.",
    fetch: (ctx) =>
      deepwikiSections(ctx, "denoland/deno").pipe(Effect.map((s) => s.filter((t) => /node/i.test(t)).slice(0, 1))),
    check: (a, f) => has(a, f[0].replace(/\s+/g, " ")),
  },
  {
    id: "two-libraries",
    suite: "heldout2",
    kind: "two independent calls",
    prompt: "Which Context7 library IDs would you use to look up documentation for React, and for Vue? Give one ID for each.",
    fetch: (ctx) =>
      Effect.all([
        ctx.tool("context7/resolve-library-id", ["--libraryName", "react", "--query", "React library documentation"]),
        ctx.tool("context7/resolve-library-id", ["--libraryName", "vue", "--query", "Vue.js framework documentation"]),
      ]).pipe(
        Effect.map(([react, vue]) => {
          const ids = (t: string) => [...t.matchAll(/library ID:\s*(\S+)/g)].map((m) => m[1]);
          // Facts: React's IDs, a separator, Vue's IDs.
          return [...ids(react), "|", ...ids(vue)];
        }),
      ),
    check: (a, f) => {
      const cut = f.indexOf("|");
      return hasAny(a, f.slice(0, cut)) && hasAny(a, f.slice(cut + 1));
    },
  },
  {
    id: "memory-years",
    suite: "heldout2",
    kind: "counting in a local server",
    prompt: "In the knowledge graph, how many entities have an observation saying they started in a year before {year}?",
    // The projects started 2015, 2016, ... one a year, so before Y there are Y - 2015 of them.
    fetch: (ctx) => Effect.succeed([String(ctx.seeded.year - 2015)]),
    check: (a, f) => hasNumber(a, f[0]),
  },
  {
    id: "newest-release",
    suite: "heldout2",
    kind: "comparing across calls",
    prompt:
      "Of the 3 most-starred repos in the modelcontextprotocol GitHub org, which one published its latest release most recently? Put just its name on the first line.",
    fetch: (ctx) =>
      Effect.gen(function* () {
        const top = yield* topRepos(ctx, "modelcontextprotocol", 3);
        const dated = yield* Effect.forEach(top, (r) =>
          ctx.gh(`repos/${r.full_name}/releases/latest`).pipe(
            Effect.map((rel) => ({ name: r.name, at: String(rel.published_at) })),
            Effect.orElseSucceed(() => ({ name: r.name, at: "" })),
          ));
        return [dated.reduce((a, b) => (b.at > a.at ? b : a)).name];
      }),
    check: (a, f) => has(firstLine(a), f[0]),
  },

  // --- held out, set 3: written and frozen before the work on calls made together (no Context7: quota) ---
  {
    id: "most-stars",
    suite: "heldout3",
    kind: "fan-out: 4 calls",
    prompt:
      "Of the GitHub repos denoland/deno, oven-sh/bun, nodejs/node and vercel/next.js, which has the most stars? Put just its owner/name on the first line.",
    fetch: (ctx) =>
      Effect.forEach(["denoland/deno", "oven-sh/bun", "nodejs/node", "vercel/next.js"], (r) =>
        ctx.gh(`repos/${r}`).pipe(Effect.map((x) => ({ r, stars: Number(x.stargazers_count) })))).pipe(
          Effect.map((xs) => [xs.reduce((a, b) => (b.stars > a.stars ? b : a)).r]),
        ),
    check: (a, f) => has(firstLine(a), f[0]),
  },
  {
    id: "three-tags",
    suite: "heldout3",
    kind: "fan-out: 3 calls",
    prompt: "What is the tag of the latest release of each of these GitHub repos: denoland/deno, oven-sh/bun and astral-sh/uv?",
    fetch: (ctx) =>
      Effect.forEach(["denoland/deno", "oven-sh/bun", "astral-sh/uv"], (r) =>
        ctx.gh(`repos/${r}/releases/latest`).pipe(Effect.map((x) => String(x.tag_name)))),
    check: (a, f) => hasAll(a, f),
  },
  {
    id: "big-repos",
    suite: "heldout3",
    kind: "a count",
    prompt: "How many repos in the modelcontextprotocol GitHub org have more than 1,000 stars?",
    fetch: (ctx) =>
      ctx.gh("search/repositories?q=org:modelcontextprotocol+stars:%3E1000&per_page=1").pipe(Effect.map((r) => [String(r.total_count)])),
    check: (a, f) => hasNumber(a, f[0]),
  },
  {
    id: "first-pr",
    suite: "heldout3",
    kind: "one call",
    prompt: "What is the title of pull request #1 in the GitHub repo modelcontextprotocol/servers?",
    fetch: (ctx) => ctx.gh("repos/modelcontextprotocol/servers/pulls/1").pipe(Effect.map((r) => [String(r.title)])),
    check: (a, f) => lower(a).replace(/[^a-z0-9]/g, "").includes(lower(f[0]).replace(/[^a-z0-9]/g, "")),
  },
  {
    id: "bun-section",
    suite: "heldout3",
    kind: "finding one section",
    prompt: "In DeepWiki's documentation of the GitHub repo oven-sh/bun, which top-level section covers the package manager? Give its title.",
    fetch: (ctx) => deepwikiSections(ctx, "oven-sh/bun").pipe(Effect.map((s) => s.filter((t) => /package/i.test(t)).slice(0, 1))),
    check: (a, f) => has(a, keyWord(f[0])),
  },
  {
    id: "started-in",
    suite: "heldout3",
    kind: "a lookup in a local server",
    prompt: "In the knowledge graph, which project started in {year}?",
    fetch: (ctx) => Effect.succeed([ctx.seeded.startedThen]),
    check: (a, f) => has(a, f[0].replace("Project ", "")),
  },
  {
    id: "newer-sdk",
    suite: "heldout3",
    kind: "fan-out: 2 calls, compare",
    prompt:
      "Which was published more recently: the latest release of modelcontextprotocol/python-sdk or the latest release of modelcontextprotocol/typescript-sdk? Put just the repo name on the first line.",
    fetch: (ctx) =>
      Effect.forEach(["python-sdk", "typescript-sdk"], (n) =>
        ctx.gh(`repos/modelcontextprotocol/${n}/releases/latest`).pipe(Effect.map((x) => ({ n, at: String(x.published_at) })))).pipe(
          Effect.map((xs) => [xs.reduce((a, b) => (b.at > a.at ? b : a)).n]),
        ),
    check: (a, f) => has(firstLine(a), f[0]) && !has(firstLine(a), f[0] === "python-sdk" ? "typescript-sdk" : "python-sdk"),
  },
  {
    id: "most-issues",
    suite: "heldout3",
    kind: "fan-out after a search",
    prompt:
      "Of the 3 most-starred repos in the denoland GitHub org, which has the most open issues (as GitHub counts them, pull requests included)? Put just its name on the first line.",
    fetch: (ctx) =>
      topRepos(ctx, "denoland", 3).pipe(Effect.map((rs) => [rs.reduce((a, b) => (b.open_issues_count > a.open_issues_count ? b : a)).name])),
    check: (a, f) => has(firstLine(a), f[0]),
  },

  // --- held out, set 4: written and frozen before the change about printing whole results (no Context7) ---
  {
    id: "ruff-issues",
    suite: "heldout4",
    kind: "a live count",
    prompt: "How many open issues does the GitHub repo astral-sh/ruff have right now, as GitHub counts them (pull requests included)?",
    fetch: (ctx) => ctx.gh("repos/astral-sh/ruff").pipe(Effect.map((r) => [String(r.open_issues_count)])),
    check: (a, f) => near(a, Number(f[0]), 0.01),
  },
  {
    id: "last-commit",
    suite: "heldout4",
    kind: "a nested field",
    prompt:
      "What is the first line of the message of the latest commit on the default branch of the GitHub repo modelcontextprotocol/modelcontextprotocol?",
    fetch: (ctx) =>
      ctx.gh("repos/modelcontextprotocol/modelcontextprotocol/commits?per_page=1").pipe(
        Effect.map((r) => [String(r[0].commit.message).split("\n")[0]]),
      ),
    check: (a, f) => lower(a).replace(/[^a-z0-9]/g, "").includes(lower(f[0]).replace(/[^a-z0-9]/g, "").slice(0, 60)),
  },
  {
    id: "pr-opener",
    suite: "heldout4",
    kind: "one call",
    prompt: "Who opened pull request #7 in the GitHub repo modelcontextprotocol/python-sdk? Give their login.",
    fetch: (ctx) => ctx.gh("repos/modelcontextprotocol/python-sdk/pulls/7").pipe(Effect.map((r) => [String(r.user.login)])),
    check: (a, f) => has(a, f[0]),
  },
  {
    id: "most-forks",
    suite: "heldout4",
    kind: "fan-out: 3 calls",
    prompt: "Which has the most forks: astral-sh/ruff, astral-sh/uv or pallets/flask? Put just its owner/name on the first line.",
    fetch: (ctx) =>
      Effect.forEach(["astral-sh/ruff", "astral-sh/uv", "pallets/flask"], (r) =>
        ctx.gh(`repos/${r}`).pipe(Effect.map((x) => ({ r, n: Number(x.forks_count) })))).pipe(
          Effect.map((xs) => [xs.reduce((a, b) => (b.n > a.n ? b : a)).r]),
        ),
    check: (a, f) => has(firstLine(a), f[0]),
  },
  {
    id: "pushed-last",
    suite: "heldout4",
    kind: "fan-out: 3 calls, compare",
    prompt:
      "Of the GitHub repos oven-sh/bun, denoland/deno and astral-sh/uv, which had a commit pushed to its default branch most recently? Put just its owner/name on the first line.",
    fetch: (ctx) =>
      Effect.forEach(["oven-sh/bun", "denoland/deno", "astral-sh/uv"], (r) =>
        ctx.gh(`repos/${r}/commits?per_page=1`).pipe(Effect.map((x) => ({ r, at: String(x[0].commit.committer.date) })))).pipe(
          Effect.map((xs) => [xs.reduce((a, b) => (b.at > a.at ? b : a)).r]),
        ),
    check: (a, f) => has(firstLine(a), f[0]),
  },
  {
    id: "ruff-section",
    suite: "heldout4",
    kind: "finding one section",
    prompt: "In DeepWiki's documentation of the GitHub repo astral-sh/ruff, which top-level section covers the formatter? Give its title.",
    fetch: (ctx) => deepwikiSections(ctx, "astral-sh/ruff").pipe(Effect.map((s) => s.filter((t) => /format/i.test(t)).slice(0, 1))),
    check: (a, f) => has(a, keyWord(f[0])),
  },
  {
    id: "entity-year",
    suite: "heldout4",
    kind: "a lookup in a local server",
    prompt: "In the knowledge graph, in what year did {entity} start?",
    fetch: (ctx) => Effect.succeed([String(ctx.seeded.entityYear)]),
    check: (a, f) => hasNumber(a, f[0]),
  },

  // --- held out, set 5: written and frozen before the accuracy fix (what each tool does). Each task needs
  // the right tool among similar ones: counts of pull requests, issues, or both; the newest issue. ---
  {
    id: "uv-open-prs",
    suite: "heldout5",
    kind: "a count of pull requests",
    prompt: "How many open pull requests does the GitHub repo astral-sh/uv have right now?",
    fetch: (ctx) => ctx.gh("search/issues?q=repo:astral-sh/uv+is:pr+is:open&per_page=1").pipe(Effect.map((r) => [String(r.total_count)])),
    check: (a, f) => numbersIn(a).some((x) => Math.abs(x - Number(f[0])) <= 2),
  },
  {
    id: "uv-open-issues",
    suite: "heldout5",
    kind: "a count of issues only",
    prompt: "How many open issues, not counting pull requests, does the GitHub repo astral-sh/uv have right now?",
    fetch: (ctx) => ctx.gh("search/issues?q=repo:astral-sh/uv+is:issue+is:open&per_page=1").pipe(Effect.map((r) => [String(r.total_count)])),
    check: (a, f) => numbersIn(a).some((x) => Math.abs(x - Number(f[0])) <= 2),
  },
  {
    id: "click-open-both",
    suite: "heldout5",
    kind: "a count of issues and pull requests",
    prompt: "How many open issues does the GitHub repo pallets/click have, as GitHub counts them with pull requests included?",
    fetch: (ctx) => ctx.gh("repos/pallets/click").pipe(Effect.map((r) => [String(r.open_issues_count)])),
    check: (a, f) => numbersIn(a).some((x) => Math.abs(x - Number(f[0])) <= 2),
  },
  {
    id: "click-stars",
    suite: "heldout5",
    kind: "a repo's numbers",
    prompt: "How many stars does the GitHub repo pallets/click have?",
    fetch: (ctx) => ctx.gh("repos/pallets/click").pipe(Effect.map((r) => [String(r.stargazers_count)])),
    check: (a, f) => near(a, Number(f[0]), 0.01),
  },
  {
    id: "flask-release-date",
    suite: "heldout5",
    kind: "a date",
    prompt: "On what date was the latest release of the GitHub repo pallets/flask published? Give it as YYYY-MM-DD.",
    fetch: (ctx) => ctx.gh("repos/pallets/flask/releases/latest").pipe(Effect.map((r) => [String(r.published_at).slice(0, 10)])),
    check: (a, f) => has(a, f[0]),
  },
  {
    id: "click-newest-issue",
    suite: "heldout5",
    kind: "sorting issues only",
    prompt: "What is the title of the most recently opened issue (not a pull request) in the GitHub repo pallets/click?",
    fetch: (ctx) =>
      ctx.gh("search/issues?q=repo:pallets/click+is:issue&sort=created&order=desc&per_page=1").pipe(
        Effect.map((r) => [String(r.items[0].title)]),
      ),
    // A leading tag such as "[rejected AI]" may fairly be left out of the answer.
    check: (a, f) =>
      lower(a).replace(/[^a-z0-9]/g, "").includes(lower(f[0].replace(/^\[[^\]]*\]\s*/, "")).replace(/[^a-z0-9]/g, "").slice(0, 50)),
  },
  {
    id: "click-testing-section",
    suite: "heldout5",
    kind: "finding one section",
    prompt: "In DeepWiki's documentation of the GitHub repo pallets/click, which top-level section covers testing? Give its title.",
    fetch: (ctx) => deepwikiSections(ctx, "pallets/click").pipe(Effect.map((s) => s.filter((t) => /test/i.test(t)).slice(0, 1))),
    check: (a, f) => has(a, keyWord(f[0])),
  },

  // --- held out, set 6: frozen before experiment 9 (limits first in what each tool does). Like set 5, on
  // repos not used before: the right count among issues, pull requests and both; the newest of each. ---
  {
    id: "httpx-open-both",
    suite: "heldout6",
    kind: "a count of issues and pull requests",
    prompt: "How many open issues does the GitHub repo encode/httpx have, as GitHub counts them with pull requests included?",
    fetch: (ctx) => ctx.gh("repos/encode/httpx").pipe(Effect.map((r) => [String(r.open_issues_count)])),
    check: (a, f) => numbersIn(a).some((x) => Math.abs(x - Number(f[0])) <= 2),
  },
  {
    id: "requests-open-both",
    suite: "heldout6",
    kind: "a count of issues and pull requests",
    prompt: "How many open issues does the GitHub repo psf/requests have, as GitHub counts them with pull requests included?",
    fetch: (ctx) => ctx.gh("repos/psf/requests").pipe(Effect.map((r) => [String(r.open_issues_count)])),
    check: (a, f) => numbersIn(a).some((x) => Math.abs(x - Number(f[0])) <= 2),
  },
  {
    id: "httpx-open-prs",
    suite: "heldout6",
    kind: "a count of pull requests",
    prompt: "How many open pull requests does the GitHub repo encode/httpx have right now?",
    fetch: (ctx) => ctx.gh("search/issues?q=repo:encode/httpx+is:pr+is:open&per_page=1").pipe(Effect.map((r) => [String(r.total_count)])),
    check: (a, f) => numbersIn(a).some((x) => Math.abs(x - Number(f[0])) <= 2),
  },
  {
    id: "requests-open-issues",
    suite: "heldout6",
    kind: "a count of issues only",
    prompt: "How many open issues, not counting pull requests, does the GitHub repo psf/requests have right now?",
    fetch: (ctx) => ctx.gh("search/issues?q=repo:psf/requests+is:issue+is:open&per_page=1").pipe(Effect.map((r) => [String(r.total_count)])),
    check: (a, f) => numbersIn(a).some((x) => Math.abs(x - Number(f[0])) <= 2),
  },
  {
    id: "httpx-newest-issue",
    suite: "heldout6",
    kind: "sorting issues only",
    prompt: "What is the title of the most recently opened issue (not a pull request) in the GitHub repo encode/httpx?",
    fetch: (ctx) =>
      ctx.gh("search/issues?q=repo:encode/httpx+is:issue&sort=created&order=desc&per_page=1").pipe(Effect.map((r) => [String(r.items[0].title)])),
    check: (a, f) =>
      lower(a).replace(/[^a-z0-9]/g, "").includes(lower(f[0].replace(/^\[[^\]]*\]\s*/, "")).replace(/[^a-z0-9]/g, "").slice(0, 50)),
  },
  {
    id: "requests-newest-pr",
    suite: "heldout6",
    kind: "sorting pull requests only",
    prompt: "What is the title of the most recently opened pull request that is still open in the GitHub repo psf/requests?",
    fetch: (ctx) =>
      ctx.gh("search/issues?q=repo:psf/requests+is:pr+is:open&sort=created&order=desc&per_page=1").pipe(Effect.map((r) => [String(r.items[0].title)])),
    check: (a, f) =>
      lower(a).replace(/[^a-z0-9]/g, "").includes(lower(f[0].replace(/^\[[^\]]*\]\s*/, "")).replace(/[^a-z0-9]/g, "").slice(0, 50)),
  },
  {
    id: "httpx-stars",
    suite: "heldout6",
    kind: "a repo's numbers (a control)",
    prompt: "How many stars does the GitHub repo encode/httpx have?",
    fetch: (ctx) => ctx.gh("repos/encode/httpx").pipe(Effect.map((r) => [String(r.stargazers_count)])),
    check: (a, f) => near(a, Number(f[0]), 0.01),
  },

  // --- multi-step: tasks that fan out to several calls, written and frozen before any run with them, and
  // reported next to the quick tasks, not instead of them ---
  {
    id: "deno-tags",
    suite: "multistep",
    kind: "a search, then 5 calls",
    prompt:
      "For each of the 5 most-starred repos in the denoland GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) =>
      Effect.gen(function* () {
        const top = yield* topRepos(ctx, "denoland", 5);
        const tags = yield* Effect.forEach(top, (r) =>
          ctx.gh(`repos/${r.full_name}/releases/latest`).pipe(
            Effect.map((rel) => String(rel.tag_name)),
            Effect.orElseSucceed(() => ""),
          ));
        return tags.filter((t) => t !== "");
      }),
    check: (a, f) => hasAll(a, f),
  },
  {
    id: "astral-issues",
    suite: "multistep",
    kind: "adding up across repos",
    prompt:
      "Across the 4 most-starred repos in the astral-sh GitHub org, how many open issues are there in total, as GitHub counts them (pull requests included)?",
    fetch: (ctx) => topRepos(ctx, "astral-sh", 4).pipe(Effect.map((rs) => [String(rs.reduce((n, r) => n + r.open_issues_count, 0))])),
    check: (a, f) => near(a, Number(f[0]), 0.01),
  },
  {
    id: "mcp-last-push",
    suite: "multistep",
    kind: "a search, then 5 calls, compare",
    prompt:
      "Of the 5 most-starred repos in the modelcontextprotocol GitHub org, which had a commit on its default branch most recently? Put just its name on the first line.",
    fetch: (ctx) =>
      Effect.gen(function* () {
        const top = yield* topRepos(ctx, "modelcontextprotocol", 5);
        const dated = yield* Effect.forEach(top, (r) =>
          ctx.gh(`repos/${r.full_name}/commits?per_page=1`).pipe(Effect.map((c) => ({ name: r.name, at: String(c[0].commit.committer.date) }))));
        const winner = dated.reduce((a, b) => (b.at > a.at ? b : a)).name;
        // Facts: the winner first, then the others.
        return [winner, ...dated.map((d) => d.name).filter((n) => n !== winner)];
      }),
    // One repo is named after the org, so drop "modelcontextprotocol/" before comparing names.
    check: (a, f) => {
      const words = lower(firstLine(a)).replaceAll("modelcontextprotocol/", "").split(/[^a-z0-9_.-]+/).filter(Boolean);
      return words.includes(lower(f[0])) && !f.slice(1).some((n) => words.includes(lower(n)));
    },
  },
  {
    id: "most-sections",
    suite: "multistep",
    kind: "3 calls, compare",
    prompt:
      "In DeepWiki's documentation, which of these GitHub repos has the most top-level sections: denoland/deno, oven-sh/bun or astral-sh/uv? Put just its owner/name on the first line.",
    fetch: (ctx) =>
      Effect.forEach(["denoland/deno", "oven-sh/bun", "astral-sh/uv"], (r) =>
        deepwikiSections(ctx, r).pipe(Effect.map((s) => ({ r, n: s.length })))).pipe(
          Effect.map((xs) => [xs.reduce((a, b) => (b.n > a.n ? b : a)).r]),
        ),
    check: (a, f) => has(firstLine(a), f[0]),
  },
  {
    id: "odd-years",
    suite: "multistep",
    kind: "filtering in a local server",
    prompt: "In the knowledge graph, list every project that started in an odd-numbered year.",
    // The projects started 2015 to 2026, one a year, in this order.
    fetch: () => Effect.succeed(["Alder", "Cedar", "Elm", "Ginkgo", "Ivy", "Larch"]),
    check: (a, f) => hasAll(a, f) && !hasAny(a, ["Birch", "Dogwood", "Fir", "Hazel", "Juniper", "Maple"]),
  },

  // --- held out, set 8: frozen before the Promise.allSettled change. Each task fans out to several calls
  // and some of them fail (a repo with no release), which is when agents fell back to calling one at a time. ---
  {
    id: "oven-tags",
    suite: "heldout8",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the oven-sh GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "oven-sh", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "encode-tags",
    suite: "heldout8",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the encode GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "encode", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "vite-tags",
    suite: "heldout8",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the vitejs GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "vitejs", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "encode-newest",
    suite: "heldout8",
    kind: "fan-out where some calls fail, compare",
    prompt:
      "Of the 5 most-starred repos in the encode GitHub org, which published its latest release most recently? Put just its name on the first line.",
    fetch: (ctx) =>
      topRepos(ctx, "encode", 5).pipe(
        Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))),
        Effect.map((xs) => [xs.filter((x) => x.at !== "").reduce((a, b) => (b.at > a.at ? b : a)).repo.split("/")[1]]),
      ),
    check: (a, f) => has(firstLine(a), f[0]),
  },
  {
    id: "mixed-tags",
    suite: "heldout8",
    kind: "5 calls, one fails",
    prompt:
      "What is the tag of the latest release of each of these GitHub repos: oven-sh/bun, vitejs/vite, astral-sh/ruff, tokio-rs/axum and encode/apistar? Say which have no release.",
    fetch: (ctx) => tagsOf(ctx, ["oven-sh/bun", "vitejs/vite", "astral-sh/ruff", "tokio-rs/axum", "encode/apistar"]).pipe(Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },

  // --- held out, set 9: frozen before experiment 12 (Promise.allSettled printing one field). Like set 8, on
  // orgs not used before: fan-out where some calls fail. ---
  {
    id: "svelte-tags",
    suite: "heldout9",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the sveltejs GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "sveltejs", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "tauri-tags",
    suite: "heldout9",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the tauri-apps GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "tauri-apps", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "nushell-tags",
    suite: "heldout9",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the nushell GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "nushell", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "hono-tags",
    suite: "heldout9",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the honojs GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "honojs", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "astro-newest",
    suite: "heldout9",
    kind: "fan-out where some calls fail, compare",
    prompt:
      "Of the 5 most-starred repos in the withastro GitHub org, which published its latest release most recently? Put just its name on the first line.",
    fetch: (ctx) =>
      topRepos(ctx, "withastro", 5).pipe(
        Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))),
        Effect.map((xs) => [xs.filter((x) => x.at !== "").reduce((a, b) => (b.at > a.at ? b : a)).repo.split("/")[1]]),
      ),
    // "astro" is inside "withastro": drop the org before comparing whole names.
    check: (a, f) => lower(firstLine(a)).replaceAll("withastro/", "").split(/[^a-z0-9_.@-]+/).includes(lower(f[0])),
  },
  // --- held out, set 10: frozen before experiment 14 (the folder README: start independent calls together,
  // print only the fields needed). Orgs and repos no earlier set used; a mix of calls that can run together
  // and single lookups, where the change should make no difference. ---
  {
    id: "biome-tags",
    suite: "heldout10",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the biomejs GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "biomejs", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "pnpm-tags",
    suite: "heldout10",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the pnpm GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "pnpm", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "bevy-tags",
    suite: "heldout10",
    kind: "fan-out where some calls fail",
    prompt: "For each of the 5 most-starred repos in the bevyengine GitHub org, what is the tag of its latest release? List each repo with its tag, and say which have no release.",
    fetch: (ctx) => topRepos(ctx, "bevyengine", 5).pipe(Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))), Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "cli-tags",
    suite: "heldout10",
    kind: "5 known calls",
    prompt:
      "What is the tag of the latest release of each of these GitHub repos: sharkdp/bat, BurntSushi/ripgrep, junegunn/fzf, ajeetdsouza/zoxide and helix-editor/helix? Say which have no release.",
    fetch: (ctx) => tagsOf(ctx, ["sharkdp/bat", "BurntSushi/ripgrep", "junegunn/fzf", "ajeetdsouza/zoxide", "helix-editor/helix"]).pipe(Effect.map(tagsAndNones)),
    check: tagsAndNonesCheck,
  },
  {
    id: "pnpm-newest",
    suite: "heldout10",
    kind: "fan-out where some calls fail, compare",
    prompt:
      "Of the 5 most-starred repos in the pnpm GitHub org, which published its latest release most recently? Put just its name on the first line.",
    fetch: (ctx) =>
      topRepos(ctx, "pnpm", 5).pipe(
        Effect.flatMap((rs) => tagsOf(ctx, rs.map((r) => r.full_name))),
        Effect.map((xs) => [xs.filter((x) => x.at !== "").reduce((a, b) => (b.at > a.at ? b : a)).repo.split("/")[1]]),
      ),
    // A repo can share the org's name (pnpm/pnpm): compare whole names, without the org.
    check: (a, f) => lower(firstLine(a)).replaceAll("pnpm/", "").split(/[^a-z0-9_.@-]+/).includes(lower(f[0])),
  },
  {
    id: "fd-issue-title",
    suite: "heldout10",
    kind: "one call, big result",
    prompt: "What is the title of issue #2 in the GitHub repo sharkdp/fd?",
    fetch: (ctx) => ctx.gh("repos/sharkdp/fd/issues/2").pipe(Effect.map((r) => [String(r.title)])),
    check: (a, f) => lower(a).replace(/[^a-z0-9]/g, "").includes(lower(f[0]).replace(/[^a-z0-9]/g, "")),
  },
  {
    id: "zed-created",
    suite: "heldout10",
    kind: "one call, exact",
    prompt: "On what date (YYYY-MM-DD) was the GitHub repo zed-industries/zed created?",
    fetch: (ctx) => ctx.gh("repos/zed-industries/zed").pipe(Effect.map((r) => [String(r.created_at).slice(0, 10)])),
    check: (a, f) => has(a, f[0]),
  },
];

/** An answer no check should accept. */
const NON_ANSWER = "I don't know.";

/**
 * A check that rejects the right answer, or accepts a non-answer, is broken: everything measured with
 * it would be wrong. Returns what is wrong, or null. The right answer is the facts, one per line.
 */
export const checkTheCheck = (task: Task, facts: Facts): string | null => {
  if (!task.check(facts.join("\n"), facts)) return `the check for ${task.id} rejects the right answer`;
  if (task.check(NON_ANSWER, facts)) return `the check for ${task.id} accepts a wrong answer`;
  return null;
};

/** The prompt with this run's seeded facts filled in. */
export const promptOf = (task: Task, seeded: Ctx["seeded"]) =>
  task.prompt.replaceAll("{word}", seeded.word).replaceAll("{year}", String(seeded.year)).replaceAll("{entity}", seeded.entity);
