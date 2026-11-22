// The benchmark tasks, and how each answer is checked against the truth fetched at the start of a run.

/** The right answers, fetched live from GitHub, DeepWiki and Context7 before any agent runs. */
export interface Truth {
  readonly latestTag: string;
  readonly top3: ReadonlyArray<string>;
  readonly sections: ReadonlyArray<string>;
  readonly context7Ids: ReadonlyArray<string>;
  /** The repo with the most open issues, counted two ways (with and without pull requests). */
  readonly mostIssues: ReadonlyArray<string>;
}

export interface Task {
  readonly id: string;
  readonly kind: string;
  readonly prompt: string;
  readonly check: (answer: string, truth: Truth) => boolean;
}

const has = (text: string, part: string) => text.toLowerCase().includes(part.toLowerCase());

/** The longest word of a section title, which a paraphrase almost always keeps. */
const keyWord = (title: string) =>
  title.split(/\W+/).reduce((best, w) => (w.length > best.length ? w : best), "");

export const TASKS: ReadonlyArray<Task> = [
  {
    id: "lookup",
    kind: "one call",
    prompt: "What is the latest release tag of the GitHub repo modelcontextprotocol/typescript-sdk?",
    check: (a, t) => has(a, t.latestTag.replace(/^v/, "")),
  },
  {
    id: "search",
    kind: "one call, filtered",
    prompt: "Which 3 repos in the modelcontextprotocol GitHub org have the most stars?",
    check: (a, t) => t.top3.every((name) => has(a, name)),
  },
  {
    id: "chain",
    kind: "two servers",
    prompt:
      "What are the top-level sections of the DeepWiki documentation for the most-starred repo in the modelcontextprotocol GitHub org?",
    check: (a, t) => {
      const hits = t.sections.filter((s) => has(a, keyWord(s))).length;
      return t.sections.length > 0 && hits >= Math.ceil((t.sections.length * 2) / 3);
    },
  },
  {
    id: "docs",
    kind: "Context7",
    prompt: "Using Context7, what is the Context7 library ID for Effect, the TypeScript library?",
    check: (a, t) => t.context7Ids.some((id) => has(a, id)),
  },
  {
    id: "compare",
    kind: "several calls",
    prompt:
      "Of the 3 most-starred repos in the modelcontextprotocol GitHub org, which one has the most open issues? Put just its name on the first line.",
    check: (a, t) => {
      const first = a.trim().split("\n")[0] ?? "";
      return t.mostIssues.some((name) => has(first, name));
    },
  },
];
