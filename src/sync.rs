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

/// `read_only`: Some(true) for --read-only, Some(false) for --all-tools, None to keep the folder's last choice.
pub fn run(source: &config::Source, root: &Path, read_only: Option<bool>) -> Result<(), String> {
    let servers = config::load(source)?;
    if servers.is_empty() {
        return Err(format!("{} lists no servers", source.describe()));
    }

    // Never write into a folder that has other things in it.
    let ours = root.join(".dogu");
    if root.exists() && !ours.exists() && fs::read_dir(root).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Err(format!("{} is not empty and was not made by dogu. Pick another --out.", root.display()));
    }
    fs::create_dir_all(&ours).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let config_path = fs::canonicalize(&source.path).map_err(|e| e.to_string())?;
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
    let old_state = saved_state(&root).unwrap_or(Value::Null);
    if let Some(old) = old_state["servers"].as_array() {
        for name in old.iter().filter_map(Value::as_str) {
            if !servers.contains_key(name) {
                let _ = fs::remove_dir_all(root.join(safe(name)));
                let _ = fs::remove_dir_all(root.join(".dogu/schemas").join(safe(name)));
            }
        }
    }

    let read_only = read_only.unwrap_or(old_state["readOnly"] == true);
    if read_only {
        println!("Read-only: keeping only the tools each server marks read-only. `--all-tools` brings the rest back.");
    }
    let width = servers.keys().map(|n| n.len()).max().unwrap_or(0);
    let mut titles = serde_json::Map::new(); // each app's name for the top README
    let mut stale = Vec::new(); // servers that did not answer; their old folder, if any, is kept
    let mut failed = 0;
    let mut need_login = false;
    // Which protocol era each server speaks, so later calls can skip the detection step.
    let mut eras = old_state["eras"].as_object().cloned().unwrap_or_default();
    eras.retain(|name, _| servers.contains_key(name));
    for (name, result) in results {
        match result {
            Ok(Fetched { info, tools, era, skipped }) => {
                let all = tools.len();
                let tools: Vec<Value> = tools
                    .into_iter()
                    .filter(|t| !read_only || crate::call::safety_of(t) == Some("read-only"))
                    .collect();
                write_server(&root, &me, name, &info, &tools)?;
                eras.insert(name.clone(), json!(era));
                match all - tools.len() {
                    0 => println!("  ok    {name:width$}  {} tools", tools.len()),
                    left => println!("  ok    {name:width$}  {} tools (left out {left} tools not marked read-only)", tools.len()),
                }
                for note in skipped {
                    println!("        left out {note}");
                }
                titles.insert(name.clone(), json!(title_of(&info)));
            }
            Err(e) => {
                failed += 1;
                println!("  FAIL  {name:width$}  {e}");
                if e.contains("dogu login") {
                    need_login = true;
                }
                // Keep the old folder, if there is one, so one broken server does not hide its tools.
                stale.push(name.clone());
            }
        }
    }

    let state = json!({
        "config": config_path, "project": source.project, "servers": servers.keys().collect::<Vec<_>>(),
        "eras": eras, "titles": titles, "stale": stale, "readOnly": read_only,
    });
    write(&ours.join("state.json"), &serde_json::to_string_pretty(&state).unwrap())?;
    write_root_readme(&root)?;
    write_stub(&root, &me, "run", "Runs a TypeScript program in the sandbox: ./run program.ts")?;
    write_stub(&root, &me, "forge", "Saves a program as a new tool: ./forge <name> program.ts")?;

    let shown = config::tilde(&root);
    println!("\nYour tools are in {shown}");
    println!("Tell your agent:  Your tools are in {shown}. Read {shown}/README.md first.");
    println!("Faster: put the list in your agent's standing instructions instead. In CLAUDE.md:  @{shown}/README.md");
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

/// One tool's line in a README: its file, its flags and the start of its description.
struct ToolLine {
    file: String,
    flags: String,
    about: String,
    /// ` → {fields}` when the tool says what it returns, else empty.
    returns: String,
    /// ` [read-only]` and the like, when the server says what the tool may change, else empty.
    safety: String,
}

/// An app in the top README: freshly synced with its tools, or kept from an earlier sync.
enum IndexEntry {
    Fresh { app: String, title: String, tools: Vec<ToolLine> },
    Stale { app: String },
}

/// The top README lists every tool with its flags while that stays under this size (about 8k tokens,
/// a fraction of what the tools' full definitions take), so an agent can usually run a tool straight
/// after reading it. Past it, descriptions go first, then flags; the last resort is names only.
const INDEX_BUDGET: usize = 32_000;

/// How much of each tool the top README shows, from most to least.
#[derive(Clone, Copy, PartialEq)]
enum Detail {
    Full,
    FlagsOnly,
    Names,
}

/// A script in the folder's root that runs a dogu command there, like `./run` and `./forge`.
fn write_stub(root: &Path, me: &Path, command: &str, about: &str) -> Result<(), String> {
    let text = format!(
        "#!/bin/sh\n# Made by dogu. {about}\nM={}; [ -x \"$M\" ] || M=dogu\nexec \"$M\" {command} --root {} \"$@\"\n",
        quote(&me.to_string_lossy()),
        quote(&root.to_string_lossy()),
    );
    write(&root.join(command), &text)?;
    make_executable(&root.join(command))
}

/// The script for one tool: it hands the work to the dogu binary.
pub fn tool_stub(me: &Path, root: &Path, server: &str, tool: &str) -> String {
    format!(
        "#!/bin/sh\n# Made by dogu. Run with --help to see the parameters.\n\
         M={}; [ -x \"$M\" ] || M=dogu\n\
         exec \"$M\" call --root {} {} {} \"$@\"\n",
        quote(&me.to_string_lossy()),
        quote(&root.to_string_lossy()),
        quote(server),
        quote(tool),
    )
}

/// A tool's line in a README, from its schema.
fn tool_line(tool: &Value) -> ToolLine {
    let about = first_line(tool["description"].as_str().unwrap_or(""));
    ToolLine {
        file: safe(tool["name"].as_str().unwrap_or("")),
        flags: crate::call::flags_of(tool),
        about: first_sentence(&about),
        returns: crate::call::returns_of(tool).map(|r| format!(" → {r}")).unwrap_or_default(),
        safety: crate::call::safety_of(tool).map(|s| format!(" [{s}]")).unwrap_or_default(),
    }
}

/// The saved schemas of an app's tools, sorted by name.
pub fn saved_schemas(root: &Path, server: &str) -> Vec<Value> {
    let dir = root.join(".dogu/schemas").join(safe(server));
    let mut list: Vec<Value> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|t| serde_json::from_str::<Value>(&t).ok())
        .collect();
    list.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    list
}

/// The top README, made from what is on disk: the synced apps and the tools agents have forged.
pub fn write_root_readme(root: &Path) -> Result<(), String> {
    let state = saved_state(root).unwrap_or(Value::Null);
    let names = |key: &str| -> Vec<String> {
        state[key].as_array().map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default()
    };
    let stale = names("stale");
    let mut index = Vec::new();
    for server in names("servers") {
        if stale.contains(&server) {
            if root.join(safe(&server)).is_dir() {
                index.push(IndexEntry::Stale { app: safe(&server) });
            }
            continue;
        }
        let title = state["titles"][&server].as_str().unwrap_or("").to_string();
        let tools = saved_schemas(root, &server).iter().map(tool_line).collect();
        index.push(IndexEntry::Fresh { app: safe(&server), title, tools });
    }
    let forged: Vec<ToolLine> = saved_schemas(root, FORGED).iter().map(tool_line).collect();
    if !forged.is_empty() {
        index.push(IndexEntry::Fresh { app: FORGED.into(), title: " (made by your agents with ./forge)".into(), tools: forged });
    }
    write(&root.join("README.md"), &root_readme(&index))?;
    write(&root.join("SKILL.md"), &skill_file(root, &index))
}

/// The Agent Skills spec's limit for a description.
const SKILL_DESCRIPTION_MAX: usize = 1024;

/// SKILL.md makes the folder an Agent Skill (agentskills.io): an agent that reads skills sees the
/// description in every session and opens README.md when a task needs one of the tools.
fn skill_file(root: &Path, index: &[IndexEntry]) -> String {
    let app = |e: &IndexEntry, detail: bool| match e {
        IndexEntry::Fresh { app, title, tools } if detail => format!("{app} ({} tools{title})", tools.len()),
        IndexEntry::Fresh { app, .. } | IndexEntry::Stale { app } => app.clone(),
    };
    let describe = |detail: bool| {
        let apps: Vec<String> = index.iter().map(|e| app(e, detail)).collect();
        format!(
            "The user's MCP tools as commands in a folder: {}. Use when a task needs one of these services, \
             or a tool an agent saved with forge. Read README.md there first.",
            apps.join(", ")
        )
        .replace(" (", " - ")
        .replace(')', "")
    };
    let description = Some(describe(true)).filter(|d| d.len() <= SKILL_DESCRIPTION_MAX).unwrap_or_else(|| describe(false));
    let description: String = description.chars().take(SKILL_DESCRIPTION_MAX - "description: ".len()).collect();
    let shown = config::tilde(root);
    format!(
        "---\nname: mcp-tools\ndescription: {}\n---\n\n# MCP tools\n\n\
         Every MCP tool the user has is a command in `{shown}`. Read `{shown}/README.md` first: it lists every\n\
         tool with its flags and the fields it returns, and says how to pipe tools together, write one program\n\
         that calls several, and save a program as a new tool.\n\n\
         Run tools from that folder, for example: `cd {shown} && <app>/<tool> --flag value --json | jq '{{id, name}}'`\n",
        // A JSON string is a valid YAML double-quoted value, so `:` and quotes inside are safe.
        serde_json::Value::String(description)
    )
}

/// Link the folder into the skill folders agents read, so it is found with no instructions at all.
/// Never replaces something already there.
pub fn link_skill(root: &Path) -> Vec<String> {
    let home = config::home();
    [".claude/skills", ".agents/skills"]
        .iter()
        .map(|dir| {
            let link = home.join(dir).join("mcp-tools");
            let shown = config::tilde(&link);
            if link.exists() || link.symlink_metadata().is_ok() {
                return match fs::canonicalize(&link).ok() == fs::canonicalize(root).ok() {
                    true => format!("  {shown} already links here"),
                    false => format!("  {shown} exists and is something else, so it was left alone"),
                };
            }
            match fs::create_dir_all(home.join(dir)).and_then(|_| symlink_dir(root, &link)) {
                Ok(()) => format!("  linked {shown}"),
                Err(e) => format!("  could not link {shown}: {e}"),
            }
        })
        .collect()
}

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink_dir(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("links are not supported here"))
}

/// The app that holds the tools agents make with `forge`.
pub const FORGED: &str = "forged";

/// Writes the app's folder: a script per tool, the saved schemas, the README and the types.
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
    readme.push_str("Each line shows a tool and its flags. Run `./<tool> --help` for what the flags mean.\n\n");

    for tool in tools {
        let Some(tool_name) = tool["name"].as_str() else { continue };
        let file = safe(tool_name);

        let path = dir.join(&file);
        write(&path, &tool_stub(me, root, name, tool_name))?;
        make_executable(&path)?;

        write(&schema_path(root, name, tool_name), &serde_json::to_string_pretty(tool).unwrap())?;

        let flags = crate::call::flags_of(tool);
        let about = first_line(tool["description"].as_str().unwrap_or(""));
        let returns = crate::call::returns_of(tool).map(|r| format!(" → {r}")).unwrap_or_default();
        let safety = crate::call::safety_of(tool).map(|s| format!(" [{s}]")).unwrap_or_default();
        readme.push_str(&format!("- `{file}{flags}` — {about}{returns}{safety}\n"));
    }
    write(&dir.join("index.d.ts"), &types_file(name, tools))?;
    write(&dir.join("README.md"), &readme)
}

/// The server's tools written as TypeScript types. An agent reads this before writing a program.
pub fn types_file(server: &str, tools: &[Value]) -> String {
    let app = ident(server);
    let mut out = format!(
        "// The \"{server}\" tools, for code mode. `tools` is a global inside a program run with `run`.\n\
         // Every call returns a Promise. A failed call throws an Error.\n\
         // Where the result type is `any`: you get parsed JSON if the tool answered with JSON, else a string.\n\
         // Example:  const result = await tools.{app}.some_tool({{ name: \"value\" }});\n\n\
         declare const tools: {{\n  {app}: {{\n"
    );
    for tool in tools {
        out.push_str(&tool_decl(tool, "    ", ""));
    }
    out.push_str("  };\n};\n");
    out
}

/// One tool as a TypeScript declaration with its doc comment, e.g. `name(args: {...}): Promise<any>;`.
/// `indent` goes before every line and `prefix` before the name, such as `tools.github.`.
pub fn tool_decl(tool: &Value, indent: &str, prefix: &str) -> String {
    let Some(name) = tool["name"].as_str() else { return String::new() };
    let mut out = String::new();
    if let Some(d) = tool["description"].as_str() {
        out.push_str(&format!("{indent}/**\n"));
        for line in d.trim().lines() {
            out.push_str(&format!("{indent} * {}\n", line.trim_end().replace("*/", "* /")));
        }
        out.push_str(&format!("{indent} */\n"));
    }
    let input = &tool["inputSchema"];
    let required: Vec<&str> =
        input["required"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let returns = if tool["outputSchema"].is_object() { ts_type(&tool["outputSchema"], 0) } else { "any".into() };
    let id = format!("{prefix}{}", ident(name));
    match input["properties"].as_object().filter(|p| !p.is_empty()) {
        None => out.push_str(&format!("{indent}{id}(args?: {{}}): Promise<{returns}>;\n\n")),
        Some(props) => {
            let optional = if required.is_empty() { "?" } else { "" };
            out.push_str(&format!("{indent}{id}(args{optional}: {{\n"));
            for (key, prop) in props {
                if let Some(d) = prop["description"].as_str() {
                    out.push_str(&format!("{indent}  /** {} */\n", d.trim().replace('\n', " ").replace("*/", "* /")));
                }
                let mark = if required.contains(&key.as_str()) { "" } else { "?" };
                out.push_str(&format!("{indent}  {}{mark}: {};\n", ts_key(key), ts_type(prop, 0)));
            }
            out.push_str(&format!("{indent}}}): Promise<{returns}>;\n\n"));
        }
    }
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

fn root_readme(index: &[IndexEntry]) -> String {
    format!(
        "# Tools as files\n\n\
         Every folder here is an app. Every file inside it is a tool you can run.\n\n\
         Every tool and its flags are listed at the end of this file, so you can usually run one\n\
         straight away. Run `<app>/<tool> --help` for what a tool's flags mean.\n\n\
         A `→ {{fields}}` after a tool means `--json` prints JSON with those fields. Pick the ones you need\n\
         with `jq` in the same command, without looking at the output first:\n\
         `<app>/<tool> --name value --json | jq '{{id, name}}'`\n\n\
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
         ## Make your own tool\n\n\
         When you have written a program that you or another agent will need again, save it as a tool:\n\
         `./forge <name> program.ts`. Start the program with a `// @description <what it does>` line and a\n\
         `// @param <name> <type> [required] <what it is>` line for each input (types: string, number,\n\
         integer, boolean, json). Inside, the inputs are in `args`. Print the result, as JSON if you can.\n\
         It then works like any other tool: `forged/<name> --flag value`, `--help`, and\n\
         `tools.forged.<name>({{...}})` in a program. It may only call the tools its code names.\n\n\
         ## Apps\n\n{}\n",
        app_index(index, INDEX_BUDGET)
    )
}

/// The apps and their tools, with as much detail as fits in `budget` bytes.
fn app_index(index: &[IndexEntry], budget: usize) -> String {
    let render = |detail| index.iter().map(|e| app_entry(e, detail)).collect::<Vec<_>>().join("\n");
    [Detail::Full, Detail::FlagsOnly]
        .into_iter()
        .map(render)
        .find(|text| text.len() <= budget)
        .unwrap_or_else(|| render(Detail::Names))
}

fn app_entry(entry: &IndexEntry, detail: Detail) -> String {
    match entry {
        IndexEntry::Stale { app } => format!("- `{app}/` — (not refreshed: server did not answer)"),
        IndexEntry::Fresh { app, title, tools } if detail == Detail::Names => {
            let names: Vec<&str> = tools.iter().map(|t| t.file.as_str()).collect();
            format!("- `{app}/` — {} tools{title}: {}", tools.len(), names.join(", "))
        }
        IndexEntry::Fresh { app, title, tools } => {
            let lines: Vec<String> = tools
                .iter()
                .map(|t| match detail {
                    Detail::Full => format!("- `{app}/{}{}` — {}{}{}", t.file, t.flags, t.about, t.returns, t.safety),
                    _ => format!("- `{app}/{}{}`{}{}", t.file, t.flags, t.returns, t.safety),
                })
                .collect();
            format!("### `{app}/` — {} tools{title}\n\n{}\n", tools.len(), lines.join("\n"))
        }
    }
}

/// The first sentence of a description, cut to 100 characters: enough to choose a tool by.
fn first_sentence(text: &str) -> String {
    let sentence = text.split(". ").next().unwrap_or(text).trim_end_matches('.');
    match sentence.chars().count() > 100 {
        true => format!("{}…", sentence.chars().take(100).collect::<String>()),
        false => format!("{sentence}."),
    }
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

/// What the last `dogu sync` saved about a folder: which config, which servers, their eras.
pub fn saved_state(root: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(root.join(".dogu/state.json")).ok()?).ok()
}

/// The era we saw for a server at the last sync, if any.
pub fn saved_era(root: &Path, server: &str) -> Option<crate::client::Era> {
    let state = saved_state(root)?;
    state["eras"][server].as_str().map(crate::client::Era::from_label)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(tools: usize, about_len: usize) -> IndexEntry {
        IndexEntry::Fresh {
            app: "gh".into(),
            title: String::new(),
            tools: (0..tools)
                .map(|i| ToolLine {
                    file: format!("tool_{i}"),
                    flags: " --repo <string>".into(),
                    about: "x".repeat(about_len),
                    returns: " → {id, name}".into(),
                    safety: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_small_index_has_flags_descriptions_and_returns() {
        let text = app_index(&[app(3, 20)], 10_000);

        assert!(text.contains("- `gh/tool_0 --repo <string>` — xxxxxxxxxxxxxxxxxxxx → {id, name}"), "{text}");
    }

    #[test]
    fn a_bigger_index_drops_descriptions_before_flags() {
        let text = app_index(&[app(40, 200)], 4_000);

        assert!(text.contains("- `gh/tool_0 --repo <string>` → {id, name}"), "{text}");
        assert!(!text.contains("xxxx"), "{text}");
    }

    #[test]
    fn a_huge_index_lists_names_only() {
        let text = app_index(&[app(400, 200)], 4_000);

        assert!(text.contains("tool_0, tool_1"), "{text}");
        assert!(!text.contains("--repo"), "{text}");
    }
}
