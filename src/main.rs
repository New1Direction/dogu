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
mod forge;
mod run;
mod serve;
mod sync;
mod util;

use std::io::Read;
use std::path::PathBuf;

const USAGE: &str = "\
dogu — your MCP tools as a folder of commands, and as a sandboxed code API

  dogu sync [--config <mcp.json>] [--out <folder>] [--link-skill]
      Ask every server in the config for its tools and write them to the folder.
      Default folder: ~/mcp. Run it again when your servers change.
      --link-skill links the folder into ~/.claude/skills and ~/.agents/skills as the skill
      `mcp-tools`, so agents that read Agent Skills find the tools on their own.

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
      Be an MCP server with search, run_code and forge. Add this to an app that has no shell.

  dogu forge [--root <folder>] [--force] <name> <program.ts | ->
      Save a program as a new tool, forged/<name>. Start it with `// @description ...` and a
      `// @param <name> <type> [required] <what it is>` line per input; inside, they are in `args`.
      It may only call the tools its code names. --force replaces a tool of the same name.

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

fn is_help(arg: &str) -> bool {
    arg == "--help" || arg == "-h"
}

/// `dogu <command> --help`. For `call`, only right after the command: a later --help is the tool's.
fn asks_for_help(command: &str, rest: &[String]) -> bool {
    match command {
        "call" => {
            let rest = if rest.first().is_some_and(|a| a == "--root") { rest.get(2..).unwrap_or_default() } else { rest };
            rest.first().is_some_and(|a| is_help(a))
        }
        _ => rest.iter().any(|a| is_help(a)),
    }
}

fn run(args: &[String]) -> Result<i32, String> {
    let default_root = config::home().join("mcp");
    let Some(command) = args.first().map(String::as_str) else {
        print!("{USAGE}");
        return Ok(0);
    };
    if matches!(command, "help" | "--help" | "-h") || asks_for_help(command, &args[1..]) {
        print!("{USAGE}");
        return Ok(0);
    }
    match command {
        "sync" => {
            let mut config_arg = None;
            let mut out = default_root;
            let mut link = false;
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--config" => config_arg = Some(it.next().ok_or("--config needs a file")?.clone()),
                    "--out" => out = PathBuf::from(it.next().ok_or("--out needs a folder")?),
                    "--link-skill" => link = true,
                    other => return Err(format!("unknown option {other}\n\n{USAGE}")),
                }
            }
            // Re-syncing a folder keeps the config it was made from, unless told otherwise.
            let told = config_arg.is_some() || std::env::var("DOGU_CONFIG").is_ok();
            let saved = sync::saved_state(&out)
                .filter(|s| !told && s["config"].as_str().is_some_and(|p| std::path::Path::new(p).is_file()));
            let source = match &saved {
                Some(state) => config::Source::from_state(state)?,
                None => config::find(config_arg.as_deref())?,
            };
            if saved.is_some() {
                println!("Reading {} (what this folder was made from. Another: --config <file>)", source.describe());
            } else {
                println!("Reading {}", source.describe());
            }
            sync::run(&source, &out)?;
            if link {
                println!("\nAgent Skills:");
                for line in sync::link_skill(&std::fs::canonicalize(&out).map_err(|e| e.to_string())?) {
                    println!("{line}");
                }
            }
            Ok(0)
        }
        "call" => {
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
        mode @ ("run" | "serve") => {
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
        mode @ ("login" | "logout") => {
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
            let source = match config_arg.as_deref() {
                Some(path) => config::find(Some(path))?,
                None => config::Source::from_state(&sync::saved_state(&default_root).unwrap_or_default())?,
            };
            let servers = config::load(&source)?;
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
        "forge" => {
            let (mut root, mut replace, mut rest) = (default_root, false, Vec::new());
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--root" => root = PathBuf::from(it.next().ok_or("--root needs a folder")?),
                    "--force" => replace = true,
                    other => rest.push(other.to_string()),
                }
            }
            let [name, file] = rest.as_slice() else {
                return Err(format!("forge needs a name and a program file (or - for stdin)\n\n{USAGE}"));
            };
            let source = match file.as_str() {
                "-" => {
                    let mut text = String::new();
                    std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
                    text
                }
                path => std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?,
            };
            println!("{}", forge::save(&root, name, &source, replace)?);
            Ok(0)
        }
        "conformance" => conformance::run(args.get(1).ok_or("conformance needs the server URL")?),
        "--version" | "-V" => {
            println!("dogu {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        other => Err(format!("unknown command {other}\n\n{USAGE}")),
    }
}
