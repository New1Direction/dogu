// This file is the only trusted code inside the sandbox. dogu puts the program's
// text where __PROGRAM__ is and runs the result in Deno with every permission off.
// The program can reach the outside world in exactly one way: the `tools` object,
// which sends each call down a pipe to dogu.
const PROGRAM = "__PROGRAM__";

const stdin = Deno.stdin, stdout = Deno.stdout, exit = Deno.exit;
const encoder = new TextEncoder();
const waiting = new Map();
let nextId = 1;

function send(message) {
  // The \x01 at the start tells dogu "this line is a tool call, not printed output".
  let bytes = encoder.encode("\x01" + JSON.stringify(message) + "\n");
  while (bytes.length > 0) bytes = bytes.subarray(stdout.writeSync(bytes));
}

// Read answers from dogu and hand each one to the call that is waiting for it.
(async () => {
  const decoder = new TextDecoder();
  const chunk = new Uint8Array(65536);
  let text = "";
  for (;;) {
    const n = await stdin.read(chunk);
    if (n === null) break;
    text += decoder.decode(chunk.subarray(0, n), { stream: true });
    let end;
    while ((end = text.indexOf("\n")) >= 0) {
      const answer = JSON.parse(text.slice(0, end));
      text = text.slice(end + 1);
      const call = waiting.get(answer.id);
      waiting.delete(answer.id);
      if (!call) continue;
      if (answer.error !== undefined) call.reject(new Error(answer.error));
      else call.resolve(answer.result);
    }
  }
})();

// tools.<app>.<tool>({ ...parameters }) returns a Promise with the result.
globalThis.tools = new Proxy({}, {
  get: (_, app) => typeof app !== "string" || app === "then" ? undefined : new Proxy({}, {
    get: (_, tool) => typeof tool !== "string" || tool === "then" ? undefined : (args = {}) =>
      new Promise((resolve, reject) => {
        const id = nextId++;
        waiting.set(id, { resolve, reject });
        send({ id, server: app, tool, args });
      }),
  }),
});

function fail(error) {
  const text = String((error && error.stack) || error)
    .replaceAll(/data:application\/typescript;base64,[A-Za-z0-9+\/=.]*/g, "program")
    .split("\n").filter((line) => !line.includes("dogu-run-")).join("\n"); // hide our own frames
  console.error(text);
  exit(1);
}
globalThis.addEventListener("unhandledrejection", (e) => { e.preventDefault(); fail(e.reason); });
globalThis.addEventListener("error", (e) => { e.preventDefault(); fail(e.error); });

try {
  // The address is put together while running, on purpose. If it were written out
  // in full, Deno would load the program's imports early, before permission checks.
  await import("data:application/typescript;base64," + PROGRAM);
} catch (error) {
  fail(error);
}
exit(0);
