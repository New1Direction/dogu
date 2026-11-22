// Runs Claude Code once, headless, and turns its event stream into numbers.

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
  readonly costUsd: number;
  /** Tokens the model read on its first turn: system prompt, tool definitions and the task. */
  readonly firstTurnTokens: number;
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

/** One headless Claude Code run. Killed after `timeout`; retried once if it produced no result. */
export const runClaude = (
  args: ReadonlyArray<string>,
  options: { readonly cwd: string; readonly env: Record<string, string>; readonly timeout: Duration.Duration },
): Effect.Effect<RunMetrics, CommandFailed | NoResult | Cause.TimeoutError> =>
  runCommand("claude", args, { cwd: options.cwd, env: options.env }).pipe(
    Effect.flatMap(metricsOf),
    Effect.timeout(options.timeout),
    Effect.retry({ times: 1, while: (e) => e._tag === "NoResult" }),
  );
