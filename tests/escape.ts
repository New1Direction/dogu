const n: number = 41;                       // TypeScript must work
console.log("ts ok", n + 1);
async function attempt(name: string, f: () => Promise<unknown> | unknown) {
  try { const r = await f(); console.log("ESCAPED", name, String(r).slice(0, 60)); }
  catch (e) { console.log("blocked", name, "-", (e as Error).name); }
}
await attempt("read file", () => Deno.readTextFile("/etc/passwd"));
await attempt("write file", () => Deno.writeTextFile("/tmp/x", "x"));
await attempt("env", () => Deno.env.get("HOME"));
await attempt("fetch", () => fetch("https://example.com"));
await attempt("fetch file", () => fetch("file:///etc/passwd"));
await attempt("subprocess", () => new Deno.Command("ls").output());
await attempt("import local ts", () => import("file:///etc/hostname"));
await attempt("import local json", () => import("file:///etc/os-release", { with: { type: "json" } }));
await attempt("import https", () => import("https://esm.sh/left-pad?leak=1"));
await attempt("import jsr", () => import("jsr:@std/path"));
await attempt("import npm", () => import("npm:left-pad"));
await attempt("node fs", async () => (await import("node:fs")).readFileSync("/etc/passwd", "utf8"));
await attempt("node child", async () => (await import("node:child_process")).execSync("id").toString());
await attempt("process.env", async () => (await import("node:process")).env.HOME);
await attempt("websocket", () => new Promise((res, rej) => { const w = new WebSocket("wss://example.com"); w.onopen = res; w.onerror = () => rej(new Error("ws")); }));
await attempt("hostname", () => Deno.hostname());
await attempt("ffi", () => (Deno as any).dlopen("libc.so.6", {}));
