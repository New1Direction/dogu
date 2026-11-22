//! `dogu sync` — ask every server for its tools and write them out as a folder.
//!
//!   <root>/README.md              how to use the folder (written for an AI agent)
//!   <root>/<server>/README.md     that server's tools, one line each
//!   <root>/<server>/<tool>        a tiny script that runs the tool
//!   <root>/<server>/index.d.ts    the same tools as TypeScript types, for code mode
//!   <root>/run                    runs a TypeScript program in the sandbox (code mode)
//!   <root>/.dogu/                our own notes: where the config is, tool schemas

use crate::client::{header_params, Client, Options};
use crate::config::{self, Server};
use crate::run::ident;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Turn any name into a safe file name.
pub fn safe(name: &str) -> String {
    let s: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect();
    if s.is_empty() || s.starts_with('.') {
        format!("_{s}")
    } else {
        s
    }
}

pub fn schema_path(root: &Path, server: &str, tool: &str) -> PathBuf {
    root.join(".dogu/schemas").join(safe(server)).join(format!("{}.json", safe(tool)))
}

pub fn run(config_path: &Path, root: &Path) -> Result<(), String> {
    let mut servers = config::load(config_path)?;
    // If this config also lists `dogu serve`, leave it out: we do not wrap ourselves.
    servers.retain(|_, s| match s {
        Server::Stdio { command, args, .. } => {
            !(Path::new(command).file_name().map_or(false, |f| f == "dogu") && args.iter().any(|a| a == "serve"))
        }
        _ => true,
    });
    if servers.is_empty() {
        return Err(format!("{} lists no servers", config_path.display()));
    }

    // Never write into a folder that has other things in it.
    let ours = root.join(".dogu");
    if root.exists() && !ours.exists() && fs::read_dir(root).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Err(format!("{} is not empty and was not made by dogu. Pick another --out.", root.display()));
    }
    fs::create_dir_all(&ours).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let config_path = fs::canonicalize(config_path).map_err(|e| e.to_string())?;
    let me = std::env::current_exe().map_err(|e| e.to_string())?;

    // Talk to all servers at the same time.
    let results: Vec<(&String, Result<Fetched, String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = servers
            .iter()
            .map(|(name, server)| (name, scope.spawn(move || fetch(server))))
            .collect();
        handles
            .into_iter()
            .map(|(name, h)| (name, h.join().unwrap_or_else(|_| Err("crashed".into()))))
            .collect()
    });

    // Remove folders of servers that are no longer in the config.
    let old_state: Value = fs::read_to_string(ours.join("state.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    if let Some(old) = old_state["servers"].as_array() {
        for name in old.iter().filter_map(Value::as_str) {
            if !servers.contains_key(name) {
                let _ = fs::remove_dir_all(root.join(safe(name)));
                let _ = fs::remove_dir_all(root.join(".dogu/schemas").join(safe(name)));
            }
        }
    }

    let width = servers.keys().map(|n| n.len()).max().unwrap_or(0);
    let mut index = Vec::new(); // lines for the top README
    let mut failed = 0;
    let mut need_login = false;
    // Which protocol era each server speaks, so later calls can skip the detection step.
    let mut eras = old_state["eras"].as_object().cloned().unwrap_or_default();
    eras.retain(|name, _| servers.contains_key(name));
    for (name, result) in results {
        match result {
            Ok(Fetched { info, tools, era, skipped }) => {
                write_server(&root, &me, name, &info, &tools)?;
                eras.insert(name.clone(), json!(era));
                println!("  ok    {name:width$}  {} tools", tools.len());
                for note in skipped {
                    println!("        left out {note}");
                }
                index.push(format!("- `{}/` — {} tools{}", safe(name), tools.len(), title_of(&info)));
            }
            Err(e) => {
                failed += 1;
                println!("  FAIL  {name:width$}  {e}");
                if e.contains("dogu login") {
                    need_login = true;
                }
                // Keep the old folder, if there is one, so one broken server does not hide its tools.
                if root.join(safe(name)).is_dir() {
                    index.push(format!("- `{}/` — (not refreshed: server did not answer)", safe(name)));
                }
            }
        }
    }

    let state = json!({ "config": config_path, "servers": servers.keys().collect::<Vec<_>>(), "eras": eras });
    write(&ours.join("state.json"), &serde_json::to_string_pretty(&state).unwrap())?;
    write(&root.join("README.md"), &root_readme(&index))?;
    let run = format!(
        "#!/bin/sh\n# Made by dogu. Runs a TypeScript program in the sandbox: ./run program.ts\n\
         M={}; [ -x \"$M\" ] || M=dogu\nexec \"$M\" run --root {} \"$@\"\n",
        quote(&me.to_string_lossy()),
        quote(&root.to_string_lossy()),
    );
    write(&root.join("run"), &run)?;
    make_executable(&root.join("run"))?;

    println!("\nYour tools are in {}", root.display());
    println!("Tell your agent:  Your tools are in {0}. Read {0}/README.md first.", root.display());
    if need_login {
        println!("Some servers need a login. Run `dogu login`, then `dogu sync` again.");
    } else if failed > 0 {
        println!("{failed} server(s) failed. Run again with DOGU_DEBUG=1 to see why.");
    }
    Ok(())
}

struct Fetched {
    info: Value,
    tools: Vec<Value>,
    era: String,
    /// Tools we refuse to use, with the reason.
    skipped: Vec<String>,
}

fn fetch(server: &Server) -> Result<Fetched, String> {
    let mut client = Client::connect(server, &Options::new(Duration::from_secs(60)))?;
    let mut tools = client.list_tools()?;
    let mut skipped = Vec::new();
    // Over HTTP a tool can ask for some parameters to be copied into HTTP headers. The spec says a
    // tool that asks for this in a broken way must be left out.
    if matches!(server, Server::Http { .. }) {
        tools.retain(|tool| match header_params(&tool["inputSchema"]) {
            Ok(_) => true,
            Err(why) => {
                skipped.push(format!("{}: {why}", tool["name"].as_str().unwrap_or("?")));
                false
            }
        });
    }
    Ok(Fetched { info: client.info.clone(), tools, era: client.era().label(), skipped })
}

fn title_of(info: &Value) -> String {
    let s = &info["serverInfo"];
    match s["title"].as_str().or(s["name"].as_str()) {
        Some(t) => format!(" ({t})"),
        None => String::new(),
    }
}

fn write_server(root: &Path, me: &Path, name: &str, info: &Value, tools: &[Value]) -> Result<(), String> {
    let dir = root.join(safe(name));
    let schemas = root.join(".dogu/schemas").join(safe(name));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&schemas);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&schemas).map_err(|e| e.to_string())?;

    let mut readme = format!("# {name}\n\n");
    if let Some(text) = info["instructions"].as_str() {
        readme.push_str(text.trim());
        readme.push_str("\n\n");
    }
    readme.push_str("Run `./<tool> --help` to see a tool's parameters.\n\n");

    for tool in tools {
        let Some(tool_name) = tool["name"].as_str() else { continue };
        let file = safe(tool_name);

        // The stub: a two-line script that hands the work to the dogu binary.
        let stub = format!(
            "#!/bin/sh\n# Made by dogu. Run with --help to see the parameters.\n\
             M={}; [ -x \"$M\" ] || M=dogu\n\
             exec \"$M\" call --root {} {} {} \"$@\"\n",
            quote(&me.to_string_lossy()),
            quote(&root.to_string_lossy()),
            quote(name),
            quote(tool_name),
        );
        let path = dir.join(&file);
        write(&path, &stub)?;
        make_executable(&path)?;

        write(&schema_path(root, name, tool_name), &serde_json::to_string_pretty(tool).unwrap())?;

        let about = first_line(tool["description"].as_str().unwrap_or(""));
        readme.push_str(&format!("- `{file}` — {about}\n"));
    }
    write(&dir.join("index.d.ts"), &types_file(name, tools))?;
    write(&dir.join("README.md"), &readme)
}

/// The server's tools written as TypeScript types. An agent reads this before writing a program.
fn types_file(server: &str, tools: &[Value]) -> String {
    let app = ident(server);
    let mut out = format!(
        "// The \"{server}\" tools, for code mode. `tools` is a global inside a program run with `run`.\n\
         // Every call returns a Promise. A failed call throws an Error.\n\
         // Where the result type is `any`: you get parsed JSON if the tool answered with JSON, else a string.\n\
         // Example:  const result = await tools.{app}.some_tool({{ name: \"value\" }});\n\n\
         declare const tools: {{\n  {app}: {{\n"
    );
    for tool in tools {
        let Some(name) = tool["name"].as_str() else { continue };
        if let Some(d) = tool["description"].as_str() {
            out.push_str("    /**\n");
            for line in d.trim().lines() {
                out.push_str(&format!("     * {}\n", line.trim_end().replace("*/", "* /")));
            }
            out.push_str("     */\n");
        }
        let input = &tool["inputSchema"];
        let required: Vec<&str> =
            input["required"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let returns = if tool["outputSchema"].is_object() { ts_type(&tool["outputSchema"], 0) } else { "any".into() };
        match input["properties"].as_object().filter(|p| !p.is_empty()) {
            None => out.push_str(&format!("    {}(args?: {{}}): Promise<{returns}>;\n\n", ident(name))),
            Some(props) => {
                let optional = if required.is_empty() { "?" } else { "" };
                out.push_str(&format!("    {}(args{optional}: {{\n", ident(name)));
                for (key, prop) in props {
                    if let Some(d) = prop["description"].as_str() {
                        out.push_str(&format!("      /** {} */\n", d.trim().replace('\n', " ").replace("*/", "* /")));
                    }
                    let mark = if required.contains(&key.as_str()) { "" } else { "?" };
                    out.push_str(&format!("      {}{mark}: {};\n", ts_key(key), ts_type(prop, 0)));
                }
                out.push_str(&format!("    }}): Promise<{returns}>;\n\n"));
            }
        }
    }
    out.push_str("  };\n};\n");
    out
}

fn ts_key(key: &str) -> String {
    if ident(key) == key {
        key.to_string()
    } else {
        Value::String(key.to_string()).to_string()
    }
}

/// Turn a JSON Schema into a TypeScript type. Anything unusual becomes `any`.
fn ts_type(schema: &Value, depth: usize) -> String {
    if depth > 5 {
        return "any".into();
    }
    if let Some(options) = schema["enum"].as_array() {
        return options.iter().map(Value::to_string).collect::<Vec<_>>().join(" | ");
    }
    if !schema["const"].is_null() {
        return schema["const"].to_string();
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(list) = schema[key].as_array() {
            return list.iter().map(|s| ts_type(s, depth + 1)).collect::<Vec<_>>().join(" | ");
        }
    }
    let kind = match &schema["type"] {
        Value::String(t) => t.as_str(),
        Value::Array(list) => {
            // e.g. ["string", "null"]
            return list
                .iter()
                .map(|t| {
                    let mut one = schema.clone();
                    one["type"] = t.clone();
                    ts_type(&one, depth + 1)
                })
                .collect::<Vec<_>>()
                .join(" | ");
        }
        _ if schema["properties"].is_object() => "object",
        _ => return "any".into(),
    };
    match kind {
        "string" => "string".into(),
        "integer" | "number" => "number".into(),
        "boolean" => "boolean".into(),
        "null" => "null".into(),
        "array" => {
            let item = ts_type(&schema["items"], depth + 1);
            if item.contains(' ') {
                format!("({item})[]")
            } else {
                format!("{item}[]")
            }
        }
        "object" => match schema["properties"].as_object().filter(|p| !p.is_empty()) {
            None => "Record<string, any>".into(),
            Some(props) => {
                let required: Vec<&str> = schema["required"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                let fields: Vec<String> = props
                    .iter()
                    .map(|(k, p)| {
                        let mark = if required.contains(&k.as_str()) { "" } else { "?" };
                        format!("{}{mark}: {}", ts_key(k), ts_type(p, depth + 1))
                    })
                    .collect();
                format!("{{ {} }}", fields.join("; "))
            }
        },
        _ => "any".into(),
    }
}

fn root_readme(index: &[String]) -> String {
    format!(
        "# Tools as files\n\n\
         Every folder here is an app. Every file inside it is a tool you can run.\n\n\
         | To do this | Run this |\n\
         |---|---|\n\
         | See the apps | `ls` |\n\
         | See an app's tools, one line each | `cat <app>/README.md` |\n\
         | Find a tool by a word | `grep -i <word> */README.md` |\n\
         | See a tool's parameters | `<app>/<tool> --help` |\n\
         | Run a tool | `<app>/<tool> --name value --other value` |\n\
         | Run a tool with JSON | `<app>/<tool> '{{\"name\": \"value\"}}'` |\n\
         | Run a tool with JSON from a pipe | `... \\| <app>/<tool> -` |\n\n\
         Results print to stdout. Errors print to stderr and the exit code is 1.\n\
         When a result may be big, send it to a file or through `head` / `jq`. Do not print all of it.\n\n\
         ## Many steps at once: write a program\n\n\
         When a job needs several tool calls, loops, or filtering of big results, write one\n\
         TypeScript program and run it with `./run`. It runs in a sandbox: it cannot read files,\n\
         use the network or start programs. It can only call tools and print.\n\n\
         ```\n\
         ./run - <<'EOF'\n\
         const items = await tools.some_app.some_tool({{ name: \"value\" }});\n\
         console.log(items.length);   // print only what you need to see\n\
         EOF\n\
         ```\n\n\
         - `tools.<app>.<tool>({{...}})` returns a Promise. A failed call throws.\n\
         - In code, write `-` and `.` in names as `_`: the tool `get-sum` is `tools.app.get_sum`.\n\
         - Types for an app's tools: `cat <app>/index.d.ts`\n\
         - Also: `./run program.ts`, and `./run --allow <app> ...` to limit which tools it may call.\n\n\
         ## Apps\n\n{}\n",
        index.join("\n")
    )
}

fn first_line(s: &str) -> String {
    let line = s.trim().lines().next().unwrap_or("").trim();
    if line.chars().count() > 160 {
        format!("{}…", line.chars().take(160).collect::<String>())
    } else {
        line.to_string()
    }
}

/// Quote a string for /bin/sh.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// The era we saw for a server at the last sync, if any.
pub fn saved_era(root: &Path, server: &str) -> Option<crate::client::Era> {
    let state: Value = serde_json::from_str(&fs::read_to_string(root.join(".dogu/state.json")).ok()?).ok()?;
    state["eras"][server].as_str().map(crate::client::Era::from_label)
}
