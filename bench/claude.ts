// Runs an agent once, headless (Claude Code or Codex), and turns its event stream into numbers.

import { Cause, Data, Duration, Effect, Option, Schema } from "npm:effect@4.1.5";

export class CommandFailed extends Data.TaggedError("CommandFailed")<{
  readonly command: string;
  readonly detail: string;
}> {}

export class NoResult extends Data.TaggedError("NoResult")<{ readonly detail: string }> {}

/** Run a program and return its stdout. A non-zero exit is a CommandFailed. Interrupting kills it. */
export const runCommand = (
  command: string,
  args: ReadonlyArray<string>,
  options: { readonly cwd?: string; readonly env?: Record<string, string> } = {},
): Effect.Effect<string, CommandFailed> =>
  Effect.tryPromise({
    try: (signal) =>
      new Deno.Command(command, {
        args: [...args],
        cwd: options.cwd,
        env: options.env,
        stdin: "null",
        stdout: "piped",
        stderr: "piped",
        signal,
      }).output(),
    catch: (e) => new CommandFailed({ command, detail: String(e) }),
  }).pipe(
    Effect.flatMap((out) => {
      const stdout = new TextDecoder().decode(out.stdout);
      return out.success
        ? Effect.succeed(stdout)
        : Effect.fail(
          new CommandFailed({ command, detail: new TextDecoder().decode(out.stderr).slice(0, 400) || stdout.slice(-400) }),
        );
    }),
  );

// --- The events we read from `claude --output-format stream-json`. Anything else is skipped. ---

const Usage = Schema.Struct({
  input_tokens: Schema.Number,
  output_tokens: Schema.Number,
  cache_creation_input_tokens: Schema.optional(Schema.Number),
  cache_read_input_tokens: Schema.optional(Schema.Number),
});

const AssistantEvent = Schema.Struct({
  type: Schema.Literal("assistant"),
  message: Schema.Struct({
    id: Schema.String,
    usage: Usage,
    content: Schema.Array(Schema.Unknown),
  }),
});

const ToolUse = Schema.Struct({
  type: Schema.Literal("tool_use"),
  name: Schema.String,
  input: Schema.Record(Schema.String, Schema.Unknown),
});

const ResultEvent = Schema.Struct({
  type: Schema.Literal("result"),
  is_error: Schema.Boolean,
  result: Schema.optional(Schema.String),
  num_turns: Schema.Number,
  duration_ms: Schema.Number,
  total_cost_usd: Schema.Number,
});

const asAssistant = Schema.decodeUnknownOption(AssistantEvent);
const asToolUse = Schema.decodeUnknownOption(ToolUse);
const asResult = Schema.decodeUnknownOption(ResultEvent);

export interface RunMetrics {
  readonly answer: string;
  readonly turns: number;
  readonly seconds: number;
  /** In dollars at API prices. Null when the agent does not report it (Codex on a ChatGPT plan). */
  readonly costUsd: number | null;
  /** Tokens the model read on its first turn: system prompt, tool definitions and the task. Null if not reported. */
  readonly firstTurnTokens: number | null;
  /** Tokens the model read over the whole run, cached or not. */
  readonly totalInputTokens: number;
  readonly outputTokens: number;
  /** Where the input tokens went: fresh, written to the prompt cache, read from it. Cache writes cost most. */
  readonly inputBreakdown: { readonly fresh: number; readonly cacheWrite: number; readonly cacheRead: number };
  readonly toolCalls: number;
  /** The agent went around its tools, e.g. curl to the GitHub API. */
  readonly bypassedTools: boolean;
  /** What the agent did, one line per tool call, in order. */
  readonly steps: ReadonlyArray<string>;
}

/** Ways around the given tools: plain HTTP, or the GitHub CLI, which is often logged in. */
const BYPASS = /\b(curl|wget)\b|api\.github\.com|(^|[\s;&|(])gh\s+(api|repo|search|issue|pr|release)\b/;

/** One readable line for a tool call: the shell command, the file read, or the MCP tool and its input. */
const stepOf = (tool: typeof ToolUse.Type): string => {
  const text = typeof tool.input.command === "string"
    ? tool.input.command
    : typeof tool.input.file_path === "string"
    ? `read ${tool.input.file_path}`
    : `${tool.name} ${JSON.stringify(tool.input)}`;
  return text.replace(/\s+/g, " ").slice(0, 240);
};

const parseLine = (line: string): unknown => {
  try {
    return JSON.parse(line);
  } catch {
    return undefined;
  }
};

/** Read the stream. Usage is counted once per model message, since one message can span several events. */
export const metricsOf = (stream: string): Effect.Effect<RunMetrics, NoResult> => {
  const events = stream.split("\n").filter((l) => l.trim() !== "").map(parseLine);
  const messages = new Map<string, typeof Usage.Type>();
  const order: Array<string> = [];
  const tools: Array<typeof ToolUse.Type> = [];
  for (const event of events) {
    const assistant = Option.getOrUndefined(asAssistant(event));
    if (assistant === undefined) continue;
    if (!messages.has(assistant.message.id)) order.push(assistant.message.id);
    messages.set(assistant.message.id, assistant.message.usage);
    for (const block of assistant.message.content) {
      const tool = Option.getOrUndefined(asToolUse(block));
      if (tool !== undefined) tools.push(tool);
    }
  }
  const result = events.map((e) => Option.getOrUndefined(asResult(e))).find((r) => r !== undefined);
  if (result === undefined) return Effect.fail(new NoResult({ detail: stream.slice(-400) }));
  if (result.is_error) return Effect.fail(new NoResult({ detail: result.result ?? "the run ended with an error" }));

  const read = (u: typeof Usage.Type) =>
    u.input_tokens + (u.cache_creation_input_tokens ?? 0) + (u.cache_read_input_tokens ?? 0);
  const usages = order.map((id) => messages.get(id)!);
  return Effect.succeed({
    answer: result.result ?? "",
    turns: result.num_turns,
    seconds: Duration.toSeconds(Duration.millis(result.duration_ms)),
    costUsd: result.total_cost_usd,
    firstTurnTokens: usages.length > 0 ? read(usages[0]) : 0,
    totalInputTokens: usages.reduce((sum, u) => sum + read(u), 0),
    outputTokens: usages.reduce((sum, u) => sum + u.output_tokens, 0),
    inputBreakdown: {
      fresh: usages.reduce((sum, u) => sum + u.input_tokens, 0),
      cacheWrite: usages.reduce((sum, u) => sum + (u.cache_creation_input_tokens ?? 0), 0),
      cacheRead: usages.reduce((sum, u) => sum + (u.cache_read_input_tokens ?? 0), 0),
    },
    toolCalls: tools.length,
    bypassedTools: tools.some((t) => typeof t.input.command === "string" && BYPASS.test(t.input.command)),
    steps: tools.map(stepOf),
  });
};

// --- Codex: `codex exec --json` ---

const CodexItem = Schema.Struct({
  type: Schema.String,
  text: Schema.optional(Schema.String),
  command: Schema.optional(Schema.String),
  server: Schema.optional(Schema.String),
  tool: Schema.optional(Schema.String),
  arguments: Schema.optional(Schema.Unknown),
});
const CodexEvent = Schema.Struct({ type: Schema.String, item: Schema.optional(CodexItem) });
const CodexUsage = Schema.Struct({
  type: Schema.Literal("turn.completed"),
  usage: Schema.Struct({
    input_tokens: Schema.Number,
    cached_input_tokens: Schema.optional(Schema.Number),
    cache_write_input_tokens: Schema.optional(Schema.Number),
    output_tokens: Schema.Number,
  }),
});
const asCodexEvent = Schema.decodeUnknownOption(CodexEvent);
const asCodexUsage = Schema.decodeUnknownOption(CodexUsage);

/**
 * Codex reports usage once per run, not per model call, and no cost on a ChatGPT plan. Turns are
 * counted as model replies: one per tool call plus the final answer.
 */
export const codexMetricsOf = (stream: string, seconds: number): Effect.Effect<RunMetrics, NoResult> => {
  const events = stream.split("\n").filter((l) => l.trim() !== "").map(parseLine);
  const items = events.flatMap((e) => {
    const ev = Option.getOrUndefined(asCodexEvent(e));
    return ev?.type === "item.completed" && ev.item ? [ev.item] : [];
  });
  const usage = events.map((e) => Option.getOrUndefined(asCodexUsage(e))).find((u) => u !== undefined)?.usage;
  const answer = items.filter((i) => i.type === "agent_message").map((i) => i.text ?? "").at(-1);
  if (usage === undefined || answer === undefined) return Effect.fail(new NoResult({ detail: stream.slice(-400) }));
  const calls = items.filter((i) => i.type === "command_execution" || i.type === "mcp_tool_call");
  const steps = calls.map((i) =>
    (i.type === "command_execution" ? i.command ?? "" : `mcp__${i.server}__${i.tool} ${JSON.stringify(i.arguments ?? {})}`)
      .replace(/\s+/g, " ").slice(0, 240)
  );
  const cached = usage.cached_input_tokens ?? 0;
  const written = usage.cache_write_input_tokens ?? 0;
  return Effect.succeed({
    answer,
    turns: calls.length + 1,
    seconds,
    costUsd: null,
    firstTurnTokens: null,
    totalInputTokens: usage.input_tokens,
    outputTokens: usage.output_tokens,
    inputBreakdown: { fresh: usage.input_tokens - cached - written, cacheWrite: written, cacheRead: cached },
    toolCalls: calls.length,
    bypassedTools: steps.some((s) => BYPASS.test(s)),
    steps,
  });
};

// --- pi: `pi --mode json` ---

const PiUsage = Schema.Struct({
  input: Schema.Number,
  output: Schema.Number,
  cacheRead: Schema.Number,
  cacheWrite: Schema.Number,
  cost: Schema.Struct({ total: Schema.Number }),
});
const PiMessageEnd = Schema.Struct({
  type: Schema.Literal("message_end"),
  message: Schema.Struct({
    role: Schema.Literal("assistant"),
    content: Schema.Array(Schema.Unknown),
    usage: PiUsage,
    stopReason: Schema.String,
    errorMessage: Schema.optional(Schema.String),
  }),
});
const PiBlock = Schema.Struct({
  type: Schema.String,
  text: Schema.optional(Schema.String),
  name: Schema.optional(Schema.String),
  arguments: Schema.optional(Schema.Record(Schema.String, Schema.Unknown)),
});
const asPiMessage = Schema.decodeUnknownOption(PiMessageEnd);
const asPiBlock = Schema.decodeUnknownOption(PiBlock);

/** pi reports each assistant message with its usage and cost; one message is one turn. */
export const piMetricsOf = (stream: string, seconds: number): Effect.Effect<RunMetrics, NoResult> => {
  const messages = stream.split("\n").filter((l) => l.trim() !== "").map(parseLine)
    .flatMap((e) => Option.toArray(asPiMessage(e)).map((m) => m.message));
  const failed = messages.find((m) => m.stopReason === "error");
  if (messages.length === 0 || failed) {
    return Effect.fail(new NoResult({ detail: failed?.errorMessage?.slice(0, 300) ?? stream.slice(-400) }));
  }
  const blocks = messages.map((m) => m.content.flatMap((b) => Option.toArray(asPiBlock(b))));
  const calls = blocks.flat().filter((b) => b.type === "toolCall");
  const steps = calls.map((b) =>
    (typeof b.arguments?.command === "string" ? b.arguments.command : `${b.name} ${JSON.stringify(b.arguments ?? {})}`)
      .replace(/\s+/g, " ").slice(0, 240)
  );
  const read = (u: typeof PiUsage.Type) => u.input + u.cacheRead + u.cacheWrite;
  const answer = blocks.at(-1)!.filter((b) => b.type === "text").map((b) => b.text ?? "").join("\n");
  return Effect.succeed({
    answer,
    turns: messages.length,
    seconds,
    costUsd: messages.reduce((sum, m) => sum + m.usage.cost.total, 0),
    firstTurnTokens: read(messages[0].usage),
    totalInputTokens: messages.reduce((sum, m) => sum + read(m.usage), 0),
    outputTokens: messages.reduce((sum, m) => sum + m.usage.output, 0),
    inputBreakdown: {
      fresh: messages.reduce((sum, m) => sum + m.usage.input, 0),
      cacheWrite: messages.reduce((sum, m) => sum + m.usage.cacheWrite, 0),
      cacheRead: messages.reduce((sum, m) => sum + m.usage.cacheRead, 0),
    },
    toolCalls: calls.length,
    bypassedTools: steps.some((s) => BYPASS.test(s)),
    steps,
  });
};

export type Agent = "claude" | "codex" | "pi";

/** One headless agent run. Killed after `timeout`; retried once if it produced no result. */
export const runAgent = (
  agent: Agent,
  bin: string,
  args: ReadonlyArray<string>,
  options: { readonly cwd: string; readonly env: Record<string, string>; readonly timeout: Duration.Duration },
): Effect.Effect<RunMetrics, CommandFailed | NoResult | Cause.TimeoutError> =>
  Effect.gen(function* () {
    const started = performance.now();
    const stream = yield* runCommand(bin, args, { cwd: options.cwd, env: options.env });
    const seconds = (performance.now() - started) / 1000;
    return yield* (agent === "codex" ? codexMetricsOf(stream, seconds) : agent === "pi" ? piMetricsOf(stream, seconds) : metricsOf(stream));
  }).pipe(
    Effect.timeout(options.timeout),
    Effect.retry({ times: 1, while: (e) => e._tag === "NoResult" }),
  );

/** One headless Claude Code run. */
export const runClaude = (
  args: ReadonlyArray<string>,
  options: { readonly cwd: string; readonly env: Record<string, string>; readonly timeout: Duration.Duration },
) => runAgent("claude", "claude", args, options);
