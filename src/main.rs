//! dogu — your MCP tools as a folder of commands, and as a sandboxed code API.
//!
//!   dogu sync     ask your MCP servers for their tools, write them out as a folder
//!   dogu call     run one tool (the generated scripts use this)
//!   dogu run      run a TypeScript program that calls tools, inside a sandbox
//!   dogu serve    be one MCP server (search + run_code) in front of all the others

mod auth;
mod call;
mod client;
mod config;
mod conformance;
mod run;
mod serve;
mod sync;
mod util;

use std::io::Read;
use std::path::PathBuf;

const USAGE: &str = "\
dogu — your MCP tools as a folder of commands, and as a sandboxed code API

  dogu sync [--config <mcp.json>] [--out <folder>]
      Ask every server in the config for its tools and write them to the folder.
      Default folder: ~/mcp. Run it again when your servers change.

  dogu login [--config <mcp.json>] [server...]
      Log in to remote servers that need it (opens your browser). With no name: all that need it.
      `dogu logout <server>` forgets a login.

  dogu call [--root <folder>] <server> <tool> [parameters]
      Run one tool. The scripts in the folder do this for you.

  dogu run [--root <folder>] [--allow <pattern>]... [--timeout <seconds>] <program.ts | - | -e 'code'>
      Run a TypeScript program in a sandbox (needs Deno). The program cannot touch files or the
      network. It can only call tools: await tools.<app>.<tool>({...})
      --allow limits the tools: `--allow github` or `--allow 'files/read_*'`. Default: all.

  dogu serve [--root <folder>] [--allow <pattern>]...
      Be an MCP server with two tools, search and run_code. Add this to an app that has no shell.

Environment:
  DOGU_CONFIG      path to the config file
  DOGU_TIMEOUT     seconds to wait (default: 300 for call, 120 for a whole program)
  DOGU_MEMORY_MB   memory limit for a program (default 256)
  DOGU_MAX_OUTPUT  most bytes a program may print (default 1000000)
  DOGU_DEBUG=1     show what stdio servers print
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    };
    std::process::exit(code);
}

fn run(args: &[String]) -> Result<i32, String> {
    let default_root = config::home().join("mcp");
    match args.first().map(String::as_str) {
        Some("sync") => {
            let mut config_arg = None;
            let mut out = default_root;
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--config" => config_arg = Some(it.next().ok_or("--config needs a file")?.clone()),
                    "--out" => out = PathBuf::from(it.next().ok_or("--out needs a folder")?),
                    other => return Err(format!("unknown option {other}\n\n{USAGE}")),
                }
            }
            let config_path = config::find(config_arg.as_deref())?;
            println!("Reading {}", config_path.display());
            sync::run(&config_path, &out)?;
            Ok(0)
        }
        Some("call") => {
            let mut rest = &args[1..];
            let mut root = default_root;
            if rest.first().map(String::as_str) == Some("--root") {
                root = PathBuf::from(rest.get(1).ok_or("--root needs a folder")?);
                rest = &rest[2..];
            }
            let [server, tool, params @ ..] = rest else {
                return Err(format!("call needs a server and a tool\n\n{USAGE}"));
            };
            call::run(&root, server, tool, params)
        }
        Some(mode @ ("run" | "serve")) => {
            let mut opts = run::Options::new(default_root);
            let mut code = None;
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--root" => opts.root = PathBuf::from(it.next().ok_or("--root needs a folder")?),
                    "--allow" => opts.allow.push(it.next().ok_or("--allow needs a pattern")?.clone()),
                    "--timeout" => {
                        let secs = it.next().and_then(|s| s.parse().ok()).ok_or("--timeout needs seconds")?;
                        opts.timeout = std::time::Duration::from_secs(secs);
                    }
                    "-e" if mode == "run" => code = Some(it.next().ok_or("-e needs code")?.clone()),
                    "-" if mode == "run" => {
                        let mut text = String::new();
                        std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
                        code = Some(text);
                    }
                    file if mode == "run" && !file.starts_with('-') => {
                        code = Some(std::fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?);
                    }
                    other => return Err(format!("unknown option {other}\n\n{USAGE}")),
                }
            }
            if mode == "serve" {
                serve::run(&opts)?;
                return Ok(0);
            }
            let code = code.ok_or("run needs a program: a file, `-` for stdin, or -e 'code'")?;
            let outcome = run::run_program(&opts, &code, &mut run::Pool::new(), false, &mut |line| println!("{line}"))?;
            Ok(outcome.code)
        }
        Some(mode @ ("login" | "logout")) => {
            let mut config_arg = None;
            let mut names = Vec::new();
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--config" => config_arg = Some(it.next().ok_or("--config needs a file")?.clone()),
                    name => names.push(name.to_string()),
                }
            }
            // Use the config the folder was made from, unless told otherwise.
            if config_arg.is_none() {
                let state: Option<serde_json::Value> = std::fs::read_to_string(default_root.join(".dogu/state.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok());
                config_arg = state.and_then(|s| s["config"].as_str().map(String::from));
            }
            let servers = config::load(&config::find(config_arg.as_deref())?)?;
            for name in &names {
                match servers.get(name) {
                    None => return Err(format!("no server \"{name}\" in the config")),
                    Some(config::Server::Stdio { .. }) => {
                        return Err(format!("{name} is a local program. Logins are for remote servers (\"url\")."))
                    }
                    _ => {}
                }
            }
            let mut failed = false;
            for (name, server) in &servers {
                let config::Server::Http { url, headers, .. } = server else { continue };
                if !names.is_empty() && !names.contains(name) {
                    continue;
                }
                if mode == "logout" {
                    let had = auth::logout(url)?;
                    println!("  {name}: {}", if had { "login forgotten" } else { "was not logged in" });
                    continue;
                }
                if headers.keys().any(|k| k.eq_ignore_ascii_case("authorization")) {
                    println!("  {name}: uses the Authorization header from your config");
                    continue;
                }
                let mut options = client::Options::new(std::time::Duration::from_secs(60));
                options.interactive = true;
                // Listing the tools makes the server ask for a login if it needs one.
                match client::Client::connect(server, &options).and_then(|mut c| c.list_tools()) {
                    Ok(_) if auth::has_login(url) => println!("  {name}: logged in"),
                    Ok(_) => println!("  {name}: needs no login"),
                    Err(e) => {
                        failed = true;
                        println!("  {name}: FAILED  {e}");
                    }
                }
            }
            if mode == "login" && !failed {
                println!("\nNow run: dogu sync");
            }
            Ok(if failed { 1 } else { 0 })
        }
        Some("conformance") => conformance::run(args.get(1).ok_or("conformance needs the server URL")?),
        Some("--version") | Some("-V") => {
            println!("dogu {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        _ => {
            print!("{USAGE}");
            Ok(0)
        }
    }
}
