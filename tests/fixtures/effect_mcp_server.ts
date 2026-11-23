// An MCP server built with Effect 4's own MCP modules (effect/ai McpServer, Tool, Toolkit), for
// tests/effect.sh. Effect's server is a separate implementation from the official SDKs, so it checks
// that dogu works with what Effect users build, in both protocol eras.
//
//   deno run -A tests/fixtures/effect_mcp_server.ts stdio 2025-11-25
//   deno run -A tests/fixtures/effect_mcp_server.ts http 2026-07-28 8765
//
// Tools: add (read-only), lookup (read-only; fails for a name it does not know), calls (how many
// times add ran: not read-only, so never answered from a cache).

import { Effect, Layer, Schema } from "npm:effect@4.1.5";
import { McpProtocol, McpServer, Tool, Toolkit } from "npm:effect@4.1.5/ai";
import { HttpRouter } from "npm:effect@4.1.5/http";
import { DenoHttpServer, DenoRuntime, DenoStdio } from "npm:@effect/platform-deno@4.1.5";

const [transport, era, port] = Deno.args;
const PROTOCOLS = {
  "2025-11-25": [McpProtocol.v2025_11_25],
  "2026-07-28": [McpProtocol.v2026_07_28],
} as const;
const protocols = PROTOCOLS[era as keyof typeof PROTOCOLS];
if (!protocols || (transport !== "stdio" && transport !== "http")) {
  console.error("usage: effect_mcp_server.ts stdio|http 2025-11-25|2026-07-28 [port]");
  Deno.exit(2);
}

const PEOPLE: Record<string, number> = { ada: 1815, grace: 1906 };
let addCalls = 0;

const Add = Tool.make("add", {
  description: "Adds two numbers.",
  parameters: Schema.Struct({ a: Schema.Number, b: Schema.Number }),
  success: Schema.Number,
}).annotate(Tool.Readonly, true);

const Lookup = Tool.make("lookup", {
  description: "Finds a person's birth year by name.",
  parameters: Schema.Struct({ name: Schema.String }),
  success: Schema.Struct({ name: Schema.String, born: Schema.Number }),
  failure: Schema.String,
}).annotate(Tool.Readonly, true);

const Calls = Tool.make("calls", {
  description: "How many times add has run on this server.",
  success: Schema.Number,
});

const Tools = Toolkit.make(Add, Lookup, Calls);

const handlers = Tools.toLayer({
  add: ({ a, b }) => Effect.sync(() => (addCalls++, a + b)),
  lookup: ({ name }) =>
    name in PEOPLE ? Effect.succeed({ name, born: PEOPLE[name] }) : Effect.fail(`no person named ${name}`),
  calls: () => Effect.sync(() => addCalls),
});

const registrations = McpServer.toolkit(Tools).pipe(Layer.provide(handlers));
const server = { name: "effect-fixture", version: "1.0.0", protocols, registrations };

const main = transport === "stdio"
  ? McpServer.layerStdio(server).pipe(Layer.provide(DenoStdio.layer))
  : HttpRouter.serve(McpServer.layerHttp({ ...server, path: "/mcp" }), { disableLogger: true, disableListenLog: true }).pipe(
    Layer.provide(DenoHttpServer.layer({ port: Number(port ?? 8765), hostname: "127.0.0.1" })),
  );

DenoRuntime.runMain(Layer.launch(main));
