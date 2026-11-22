// deno test bench/bench_test.ts

import { Effect, Result } from "npm:effect@4.1.5";
import { assert, assertEquals } from "jsr:@std/assert@1";
import { codexMetricsOf, metricsOf, piMetricsOf, timelineOf } from "./claude.ts";
import { headline, type RunRecord } from "./report.ts";
import { checkTheCheck, TASKS } from "./tasks.ts";

const line = (v: unknown) => JSON.stringify(v);
const usage = (input: number, created: number, read: number, output: number) => ({
  input_tokens: input,
  cache_creation_input_tokens: created,
  cache_read_input_tokens: read,
  output_tokens: output,
});
const assistant = (id: string, u: ReturnType<typeof usage>, content: unknown[]) =>
  line({ type: "assistant", message: { id, usage: u, content } });
const result = (text: string) =>
  line({ type: "result", is_error: false, result: text, num_turns: 2, duration_ms: 12_500, total_cost_usd: 0.05 });

const run = <A, E>(e: Effect.Effect<A, E>) => Effect.runSync(Effect.result(e));

Deno.test("counts each model message once, even when it spans several events", () => {
  const stream = [
    line({ type: "system", subtype: "init" }),
    assistant("m1", usage(2, 1000, 0, 10), [{ type: "text", text: "looking" }]),
    assistant("m1", usage(2, 1000, 0, 10), [{ type: "tool_use", name: "Bash", input: { command: "ls" } }]),
    assistant("m2", usage(3, 50, 1000, 20), [{ type: "text", text: "done" }]),
    result("the answer"),
  ].join("\n");

  const out = run(metricsOf(stream));

  assert(Result.isSuccess(out));
  assertEquals(out.success.firstTurnTokens, 1002);
  assertEquals(out.success.totalInputTokens, 1002 + 1053);
  assertEquals(out.success.outputTokens, 30);
  assertEquals(out.success.toolCalls, 1);
  assertEquals(out.success.seconds, 12.5);
  assertEquals(out.success.answer, "the answer");
  assertEquals(out.success.bypassedTools, false);
  assertEquals(out.success.steps, ["ls"]);
  assertEquals(out.success.inputBreakdown, { fresh: 5, cacheWrite: 1050, cacheRead: 1000 });
});

Deno.test("flags an agent that goes around its tools with curl", () => {
  const stream = [
    assistant("m1", usage(1, 1, 0, 1), [
      { type: "tool_use", name: "Bash", input: { command: "curl -s https://api.github.com/orgs/x/repos" } },
    ]),
    result("x"),
  ].join("\n");

  const out = run(metricsOf(stream));

  assert(Result.isSuccess(out) && out.success.bypassedTools);
});

Deno.test("flags the GitHub CLI as going around the tools", () => {
  const stream = [
    assistant("m1", usage(1, 1, 0, 1), [
      { type: "tool_use", name: "Bash", input: { command: "gh api repos/x/y/releases/latest --jq .tag_name" } },
    ]),
    result("x"),
  ].join("\n");
  const fine = [
    assistant("m1", usage(1, 1, 0, 1), [{ type: "tool_use", name: "Bash", input: { command: "github/search --q high" } }]),
    result("x"),
  ].join("\n");

  const out = run(metricsOf(stream));
  const ok = run(metricsOf(fine));

  assert(Result.isSuccess(out) && out.success.bypassedTools);
  assert(Result.isSuccess(ok) && !ok.success.bypassedTools);
});

Deno.test("reads a Codex stream: steps, usage, the last message as the answer, no cost", () => {
  const stream = [
    line({ type: "thread.started", thread_id: "t" }),
    line({ type: "item.completed", item: { id: "0", type: "agent_message", text: "Looking." } }),
    line({ type: "item.completed", item: { id: "1", type: "command_execution", command: "/bin/zsh -lc 'github/x --json'", exit_code: 0 } }),
    line({ type: "item.completed", item: { id: "2", type: "mcp_tool_call", server: "deepwiki", tool: "read_wiki_structure", arguments: { repoName: "a/b" } } }),
    line({ type: "item.completed", item: { id: "3", type: "agent_message", text: "The answer is 13." } }),
    line({ type: "turn.completed", usage: { input_tokens: 1000, cached_input_tokens: 600, cache_write_input_tokens: 100, output_tokens: 50 } }),
  ].join("\n");

  const out = run(codexMetricsOf(stream, 4.5));

  assert(Result.isSuccess(out));
  assertEquals(out.success.answer, "The answer is 13.");
  assertEquals(out.success.costUsd, null);
  assertEquals(out.success.toolCalls, 2);
  assertEquals(out.success.turns, 3);
  assertEquals(out.success.inputBreakdown, { fresh: 300, cacheWrite: 100, cacheRead: 600 });
  assertEquals(out.success.steps[1], 'mcp__deepwiki__read_wiki_structure {"repoName":"a/b"}');
});

const piMessage = (content: unknown[], usage: { input: number; output: number; cacheRead: number; cacheWrite: number; cost: number }, stopReason = "stop") =>
  line({
    type: "message_end",
    message: { role: "assistant", content, stopReason, usage: { ...usage, totalTokens: 0, cost: { total: usage.cost } } },
  });

Deno.test("reads a pi stream: one assistant message per turn, cost summed", () => {
  const stream = [
    line({ type: "session", version: 3 }),
    line({ type: "message_end", message: { role: "user", content: [{ type: "text", text: "q" }] } }),
    piMessage([{ type: "toolCall", id: "1", name: "bash", arguments: { command: "github/x --json | jq .id" } }], { input: 10, output: 5, cacheRead: 3000, cacheWrite: 200, cost: 0.002 }),
    piMessage([{ type: "text", text: "The id is 7." }], { input: 4, output: 9, cacheRead: 3200, cacheWrite: 50, cost: 0.001 }),
  ].join("\n");

  const out = run(piMetricsOf(stream, 3));

  assert(Result.isSuccess(out));
  assertEquals(out.success.answer, "The id is 7.");
  assertEquals(out.success.turns, 2);
  assertEquals(out.success.firstTurnTokens, 3210);
  assertEquals(out.success.costUsd, 0.003);
  assertEquals(out.success.steps, ["github/x --json | jq .id"]);
});

Deno.test("a pi run that errors, like an expired login, is a NoResult with the reason", () => {
  const stream = piMessage([], { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0 }, "error")
    .replace('"stopReason":"error"', '"stopReason":"error","errorMessage":"Refresh token expired"');

  const out = run(piMetricsOf(stream, 1));

  assert(Result.isFailure(out) && out.failure._tag === "NoResult" && out.failure.detail.includes("Refresh token expired"));
});

Deno.test("a stream with no result, or an error result, is a NoResult", () => {
  const none = run(metricsOf(assistant("m1", usage(1, 1, 0, 1), [])));
  const failed = run(metricsOf(line({ type: "result", is_error: true, result: "boom", num_turns: 1, duration_ms: 1, total_cost_usd: 0 })));

  assert(Result.isFailure(none) && none.failure._tag === "NoResult");
  assert(Result.isFailure(failed) && failed.failure._tag === "NoResult");
});

const check = (id: string, answer: string, facts: ReadonlyArray<string>) => TASKS.find((t) => t.id === id)!.check(answer, facts);

Deno.test("tuning checks accept right answers and reject wrong ones", () => {
  assert(check("lookup", "The latest release is 2.4.0.", ["v2.4.0"]));
  assert(!check("lookup", "The latest release is 2.3.9.", ["v2.4.0"]));
  assert(check("search", "1. servers 2. python-sdk 3. typescript-sdk", ["servers", "python-sdk", "typescript-sdk"]));
  assert(!check("search", "servers, python-sdk, inspector", ["servers", "python-sdk", "typescript-sdk"]));
  const sections = ["Introduction to Model Context Protocol Servers", "Reference Servers Overview", "Development and Contribution", "Glossary"];
  assert(check("chain", "Introduction, Reference servers, Contribution guide, Glossary", sections));
  assert(!check("chain", "Overview and Glossary", sections));
  assert(check("docs", "The ID is /effect-ts/effect.", ["/effect-ts/effect", "/effect-ts/website"]));
  assert(!check("docs", "The ID is /effect/core.", ["/effect-ts/effect"]));
  assert(check("compare", "typescript-sdk\nIt has 705 open issues.", ["typescript-sdk"]));
  assert(!check("compare", "python-sdk\ntypescript-sdk has fewer.", ["typescript-sdk"]));
});

Deno.test("held-out checks are strict", () => {
  assert(check("created", "It was created on 2021-05-28.", ["2021-05-28"]));
  assert(!check("created", "It was created in May 2021.", ["2021-05-28"]));
  assert(check("issue-title", 'The title is "Add `--port` flag".', ["Add --port flag"]));
  assert(!check("issue-title", "The title is Add flag.", ["Add --port flag"]));
  assert(check("release-tags", "servers: none\npython-sdk: v1.20.0\ntypescript-sdk: 2.4.0", ["v1.20.0", "v2.4.0"]));
  assert(!check("release-tags", "python-sdk: v1.20.0 only", ["v1.20.0", "v2.4.0"]));
  assert(check("memory-find", "It is Project Cedar.", ["Project Cedar"]));
  assert(!check("memory-find", "It is Project Birch.", ["Project Cedar"]));
  // A number must match as a whole number: 12 is not 120.
  assert(check("tokio-sections", "It has 12 top-level sections.", ["12"]));
  assert(!check("tokio-sections", "It has 120 top-level sections.", ["12"]));
  assert(check("axum-releases", "axum has published 87 releases.", ["87"]));
  assert(!check("axum-releases", "axum has published 870 releases.", ["87"]));
});

Deno.test("every held-out task is in the held-out suite, and the suites do not share ids", () => {
  const ids = TASKS.map((t) => t.id);
  assertEquals(new Set(ids).size, ids.length);
  assertEquals(TASKS.filter((t) => t.suite === "heldout").length, 8);
  assertEquals(TASKS.filter((t) => t.suite === "tuning").length, 5);
  assertEquals(TASKS.filter((t) => t.suite === "heldout2").length, 8);
});

Deno.test("headline says more, not negative fewer, when a setup used more", () => {
  const rec = (setup: string, tokens: number, cost: number): RunRecord => ({
    setup,
    task: "t",
    suite: "heldout",
    rep: 1,
    correct: true,
    metrics: { answer: "", turns: 1, seconds: 1, costUsd: cost, firstTurnTokens: tokens, totalInputTokens: tokens, outputTokens: 1, toolCalls: 0, bypassedTools: false, steps: [], inputBreakdown: { fresh: 0, cacheWrite: 0, cacheRead: 0 } },
  });

  const text = headline([rec("direct", 100, 1), rec("folder", 50, 0.5), rec("codemode", 120, 1.2)], ["direct", "folder", "codemode"]);

  assert(text.includes("50% fewer input tokens, 50% lower cost"), text);
  assert(text.includes("20% more input tokens, 20% higher cost"), text);
});

/** A plausible right answer for every task, shaped like what fetch returns. */
const SAMPLE_FACTS: Record<string, ReadonlyArray<string>> = {
  lookup: ["v2.4.0"],
  search: ["servers", "python-sdk", "typescript-sdk"],
  chain: ["Introduction to Model Context Protocol Servers", "Reference Servers Overview", "Glossary"],
  docs: ["/effect-ts/effect", "/effect-ts/website"],
  compare: ["typescript-sdk", "python-sdk"],
  created: ["2021-05-30"],
  "issue-title": ["Progress notifications"],
  "release-tags": ["v2.5.0", "2.16.0", "2026-07-28"],
  "docs-id": ["/vercel/next.js", "/websites/nextjs"],
  "deno-sections": ["Deno Overview", "CLI Architecture", "Runtime Core"],
  "memory-find": ["Project Maple"],
  "tokio-sections": ["13"],
  "axum-releases": ["175"],
  "open-prs": ["31"],
  "oldest-issue": ["Add support for resource templates"],
  "release-author": ["octocat"],
  "stars-sum": ["123456"],
  "deno-node": ["Node.js Compatibility Layer"],
  "deepwiki-node": ["Node.js Compatibility Layer"],
  "two-libraries": ["/facebook/react", "/websites/react_dev", "|", "/vuejs/core", "/websites/vuejs"],
  "memory-years": ["4"],
  "newest-release": ["typescript-sdk"],
};

Deno.test("every task's check accepts its right answer and rejects a non-answer", () => {
  for (const task of TASKS) {
    const facts = SAMPLE_FACTS[task.id];
    assert(facts, `no sample facts for ${task.id}`);
    assertEquals(checkTheCheck(task, facts), null, task.id);
  }
});

Deno.test("heldout2 checks are strict where they must be and tolerant where counts move", () => {
  assert(check("open-prs", "There are 33 open pull requests.", ["31"]));
  assert(!check("open-prs", "There are 40 open pull requests.", ["31"]));
  assert(check("stars-sum", "In total 123,900 stars.", ["123456"]));
  assert(!check("stars-sum", "In total 140,000 stars.", ["123456"]));
  assert(check("two-libraries", "React: /facebook/react, Vue: /vuejs/core", ["/facebook/react", "|", "/vuejs/core"]));
  assert(!check("two-libraries", "React: /facebook/react only", ["/facebook/react", "|", "/vuejs/core"]));
  assert(check("memory-years", "4 entities started before 2019.", ["4"]));
  // Agents often write small numbers out, and that is a right answer.
  assert(check("memory-years", "Eight entities started before 2023.", ["8"]));
  assert(!check("memory-years", "Eighteen entities.", ["8"]));
  assert(!check("memory-years", "14 entities.", ["4"]));
  assert(check("newest-release", "typescript-sdk\nreleased last week", ["typescript-sdk"]));
  assert(!check("newest-release", "servers\ntypescript-sdk was earlier", ["typescript-sdk"]));
});

Deno.test("checkTheCheck catches a check that accepts anything, and one that rejects everything", () => {
  const base = TASKS[0];
  const lenient = { ...base, check: () => true };
  const strict = { ...base, check: () => false };

  assert(checkTheCheck(lenient, ["v1"])?.includes("accepts a wrong answer"));
  assert(checkTheCheck(strict, ["v1"])?.includes("rejects the right answer"));
});

Deno.test("a run's timeline splits start-up, model and tool time by when events arrived", () => {
  const events = [
    { type: "system", subtype: "init" },
    { type: "assistant", message: { content: [{ type: "text" }] } },
    { type: "assistant", message: { content: [{ type: "tool_use" }] } },
    { type: "user", message: { content: [{ type: "tool_result" }] } },
    { type: "assistant", message: { content: [{ type: "tool_use" }] } },
    { type: "user", message: { content: [{ type: "tool_result" }] } },
    { type: "assistant", message: { content: [{ type: "text" }] } },
    { type: "result" },
  ];
  const times = [0.5, 1.0, 1.5, 2.0, 3.0, 3.25, 3.5, 4.0];

  const timeline = timelineOf(events, times);

  assertEquals(timeline, [
    { kind: "startup", seconds: 0.5 },
    { kind: "model", seconds: 1.0 },
    { kind: "tools", seconds: 0.5 },
    { kind: "model", seconds: 1.0 },
    { kind: "tools", seconds: 0.25 },
    { kind: "model", seconds: 0.75 },
  ]);
});
