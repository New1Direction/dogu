//! The sandbox on djs, a JavaScript and TypeScript engine written in Rust, instead of Deno.
//! Built with `cargo build --features djs`; `DOGU_SANDBOX=deno` still picks Deno.
//!
//! djs has no file, network, environment or process APIs at all: a program's only way out is the
//! host functions below. Every program runs under djs's own budgets (memory, fuel, a deadline),
//! which a script cannot catch.

use crate::config;
use crate::run::{tool_call, Names, Options, Outcome, Pool};
use djs_embed::{Error, HostFn, Limits, Snapshot, Source, Step, Value};
use serde_json::{json, Value as Json};

const CALL: u32 = 1; // async: tools.<app>.<tool>(args)
const PRINT: u32 = 2; // sync: console.log and friends
const EPRINT: u32 = 3; // sync: console.error and console.warn

/// Fuel is djs's instruction budget (about 4 ns a unit). The deadline is what limits a program;
/// fuel only has to be large enough never to stop one first.
const FUEL: u64 = 1 << 40;
/// How often djs hands control back between instructions, in fuel.
const SLICE: u64 = 200_000;

/// The globals a program sees: `tools` and `console`. Setup code, so it calls no host function.
const SHIM: &str = r#"
const call = (app, tool) => async (args) => {
  const answer = JSON.parse(await __dogu_call(app, tool, JSON.stringify(args ?? {})));
  if ("error" in answer) throw new Error(answer.error);
  return answer.value;
};
// `then` is left undefined, so that awaiting `tools` or `tools.app` by mistake does not call a tool.
globalThis.tools = new Proxy({}, {
  get: (_, app) => typeof app !== "string" || app === "then" ? undefined : new Proxy({}, {
    get: (_, tool) => typeof tool !== "string" || tool === "then" ? undefined : call(app, tool),
  }),
});
const show = (xs) => xs.map((x) => typeof x === "string" ? x : x instanceof Error ? String(x) : JSON.stringify(x)).join(" ");
globalThis.console = {
  log: (...xs) => __dogu_print(show(xs)),
  info: (...xs) => __dogu_print(show(xs)),
  debug: (...xs) => __dogu_print(show(xs)),
  error: (...xs) => __dogu_eprint(show(xs)),
  warn: (...xs) => __dogu_eprint(show(xs)),
};
"#;

/// Run `code` in djs. Each line it prints is handed to `out`; the shape of the result matches Deno's.
pub fn run_program(
    opts: &Options,
    code: &str,
    pool: &mut Pool,
    collect_stderr: bool,
    out: &mut dyn FnMut(&str),
) -> Result<Outcome, String> {
    let state = crate::sync::saved_state(&opts.root)
        .ok_or_else(|| format!("{} is not a dogu folder. Run `dogu sync` first.", opts.root.display()))?;
    let servers = config::load(&config::Source::from_state(&state)?)?;
    let memory = opts.memory_mb as usize * 1024 * 1024;

    // The program becomes the body of an async function, so top-level await works. It starts on
    // line 1, so the line numbers in errors are the program's own.
    let program = format!("export async function main() {{ {code}\n}}\n");
    let snapshot = Snapshot::builder()
        .host_fn(HostFn::asynchronous(CALL, "__dogu_call"))
        .host_fn(HostFn::sync(PRINT, "__dogu_print"))
        .host_fn(HostFn::sync(EPRINT, "__dogu_eprint"))
        .source(Source::script("dogu-shim.js", SHIM))
        .source(Source::typescript_module("program.ts", program))
        .memory(memory)
        .build();
    let mut stderr = String::new();
    let note = |text: String, stderr: &mut String| {
        if collect_stderr {
            stderr.push_str(&text);
            stderr.push('\n');
        } else {
            eprintln!("{text}");
        }
    };
    let snapshot = match snapshot {
        Ok(s) => s,
        Err(e) => {
            note(format!("error: {e}"), &mut stderr);
            return Ok(Outcome { code: 1, stderr });
        }
    };
    let limits = Limits { memory, fuel: FUEL, slice: SLICE, deadline: Some(opts.timeout) };
    let mut vm = snapshot.instantiate(limits).map_err(|e| e.to_string())?;

    let mut names = Names::default();
    let mut printed = 0usize;
    let finished = (|| -> Result<(), Error> {
        vm.start_export("program.ts", "main", &[])?;
        loop {
            match vm.step()? {
                Step::Done => return vm.result().map(|_| ()),
                Step::Yield => {}
                Step::HostCall(id) => {
                    let call = vm.host_call(id)?;
                    let text = call.arg(0).as_str().map(|s| s.into_owned()).unwrap_or_default();
                    if call.fn_id() == PRINT {
                        if printed <= opts.max_output {
                            printed += text.len() + 1;
                            out(&text);
                            if printed > opts.max_output {
                                out(&format!(
                                    "[output cut after {} bytes. Print less: filter or summarize in the program.]",
                                    opts.max_output
                                ));
                            }
                        }
                    } else {
                        note(text, &mut stderr);
                    }
                    call.reply(Ok(Value::Undefined))?;
                }
                Step::Wait(started) => {
                    for id in started {
                        let msg = {
                            let call = vm.host_call(id)?;
                            let arg = |i| call.arg(i).as_str().map(|s| s.into_owned()).unwrap_or_default();
                            let args: Json = serde_json::from_str(&arg(2)).unwrap_or(Json::Null);
                            json!({ "server": arg(0), "tool": arg(1), "args": args })
                        };
                        let answer = match tool_call(opts, &servers, pool, &mut names, &msg) {
                            Ok(value) => json!({ "value": value }),
                            Err(e) => json!({ "error": e }),
                        };
                        vm.resolve(id, Ok(Value::Str(&answer.to_string())))?;
                    }
                }
            }
        }
    })();

    let code = match finished {
        Ok(()) => 0,
        Err(Error::Deadline) => {
            note(format!("error: the program was stopped after {} seconds", opts.timeout.as_secs()), &mut stderr);
            124
        }
        Err(Error::OutOfMemory) => {
            note(format!("error: the program used more than {} MB of memory", opts.memory_mb), &mut stderr);
            1
        }
        Err(Error::Exception(e)) => {
            // djs's stack has the frames only; the message goes first, as in Deno.
            let text = match e.stack {
                Some(stack) if !stack.starts_with(&e.message) => format!("{}\n{stack}", e.message),
                Some(stack) => stack,
                None => e.message,
            };
            note(text, &mut stderr);
            1
        }
        Err(e) => {
            note(format!("error: {e}"), &mut stderr);
            1
        }
    };
    Ok(Outcome { code, stderr })
}
