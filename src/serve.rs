//! `dogu serve` — code mode for apps that have no shell (Claude Desktop and others).
//!
//! We act as one MCP server with two tools. All your real servers hide behind it:
//!   search    find tools and read their types
//!   run_code  run a TypeScript program in the sandbox

use crate::run::{ident, run_program, Options, Pool};
use crate::sync::safe;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const MODERN: &str = crate::client::MODERN;
/// The MCP versions we answer to: the modern one first, then the legacy ones.
const VERSIONS: [&str; 4] = [MODERN, "2025-11-25", "2025-06-18", "2025-03-26"];

const INSTRUCTIONS: &str = "\
This server gives you many tools through two. First call `search` to find tools and read their \
TypeScript types. Then call `run_code` with one TypeScript program that calls them as \
`await tools.<app>.<tool>({...})`. Do several steps in one program, and print only what you need to see.";

pub fn run(opts: &Options) -> Result<(), String> {
    if !opts.root.join(".dogu/state.json").is_file() {
        return Err(format!("{} is not an dogu folder. Run `dogu sync` first.", opts.root.display()));
    }
    let mut pool = Pool::new(); // connections stay open between programs, so servers stay warm
    let stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(method) = msg["method"].as_str() else { continue };
        if msg.get("id").is_none() {
            continue; // a notification: nothing to answer
        }
        // A modern client puts its protocol version on every request. A legacy one starts with
        // `initialize`. We answer both.
        let modern = msg["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str();
        if let Some(version) = modern.filter(|v| *v != MODERN) {
            let reply = json!({ "jsonrpc": "2.0", "id": msg["id"], "error": {
                "code": -32022, "message": "Unsupported protocol version",
                "data": { "supported": VERSIONS, "requested": version } } });
            let mut out = stdout.lock();
            let _ = writeln!(out, "{reply}").and_then(|_| out.flush());
            continue;
        }
        let result = match method {
            "server/discover" => Ok(json!({
                "supportedVersions": VERSIONS,
                "capabilities": { "tools": {} },
                "_meta": { "io.modelcontextprotocol/serverInfo": { "name": "dogu", "version": env!("CARGO_PKG_VERSION") } },
                "instructions": INSTRUCTIONS,
                "ttlMs": 3_600_000,
                "cacheScope": "private",
            })),
            "initialize" => Ok(json!({
                "protocolVersion": msg["params"]["protocolVersion"].as_str().filter(|v| VERSIONS[1..].contains(v)).unwrap_or(VERSIONS[1]),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "dogu", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": [
                {
                    "name": "search",
                    "description": "Find tools. With no parameters: list the apps. With `app`: that app's tools as \
                                    TypeScript types. With `query`: tools whose name or description has those words.",
                    "inputSchema": { "type": "object", "properties": {
                        "app": { "type": "string", "description": "An app name from the list" },
                        "query": { "type": "string", "description": "Words to look for" }
                    } }
                },
                {
                    "name": "run_code",
                    "description": "Run a TypeScript program in a sandbox. It cannot use files or the network. It can \
                                    call tools with `await tools.<app>.<tool>({...})` and print with console.log. \
                                    Returns what the program printed.",
                    "inputSchema": { "type": "object", "required": ["code"], "properties": {
                        "code": { "type": "string", "description": "The TypeScript program" }
                    } }
                }
            ] })),
            "tools/call" => {
                let args = &msg["params"]["arguments"];
                let answer = match msg["params"]["name"].as_str() {
                    Some("search") => search(opts, args["app"].as_str(), args["query"].as_str()),
                    Some("run_code") => run_code(opts, &mut pool, args["code"].as_str().unwrap_or("")),
                    _ => Err("no such tool".to_string()),
                };
                let (text, failed) = match answer {
                    Ok(text) => (text, false),
                    Err(text) => (text, true),
                };
                Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": failed }))
            }
            _ => Err(json!({ "code": -32601, "message": "method not found" })),
        };
        let reply = match result {
            Ok(mut r) => {
                if modern.is_some() {
                    r["resultType"] = json!("complete");
                    if method == "tools/list" {
                        r["ttlMs"] = json!(3_600_000);
                        r["cacheScope"] = json!("private");
                    }
                }
                json!({ "jsonrpc": "2.0", "id": msg["id"], "result": r })
            }
            Err(e) => json!({ "jsonrpc": "2.0", "id": msg["id"], "error": e }),
        };
        let mut out = stdout.lock();
        if writeln!(out, "{reply}").and_then(|_| out.flush()).is_err() {
            break;
        }
    }
    Ok(())
}

fn run_code(opts: &Options, pool: &mut Pool, code: &str) -> Result<String, String> {
    let mut text = String::new();
    let outcome = run_program(opts, code, pool, true, &mut |line| {
        text.push_str(line);
        text.push('\n');
    })?;
    text.push_str(&outcome.stderr);
    if outcome.code == 0 {
        Ok(if text.is_empty() { "(the program printed nothing)".into() } else { text })
    } else {
        Err(text)
    }
}

fn search(opts: &Options, app: Option<&str>, query: Option<&str>) -> Result<String, String> {
    let state: Value = std::fs::read_to_string(opts.root.join(".dogu/state.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    let servers: Vec<&str> = state["servers"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    // Each app's tools, as (app, tool, first line of description).
    let tools_of = |server: &str| -> Vec<(String, String)> {
        let mut list = Vec::new();
        let dir = opts.root.join(".dogu/schemas").join(safe(server));
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let Some(schema) = std::fs::read_to_string(entry.path()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
            else {
                continue;
            };
            let about = schema["description"].as_str().unwrap_or("").trim().lines().next().unwrap_or("").chars().take(160).collect::<String>();
            list.push((ident(schema["name"].as_str().unwrap_or("")), about));
        }
        list.sort();
        list
    };

    if let Some(app) = app {
        let server = servers
            .iter()
            .find(|s| **s == app || ident(s) == app)
            .ok_or_else(|| format!("no app \"{app}\". Apps: {}", servers.iter().map(|s| ident(s)).collect::<Vec<_>>().join(", ")))?;
        if query.is_none() {
            return std::fs::read_to_string(opts.root.join(safe(server)).join("index.d.ts")).map_err(|e| e.to_string());
        }
    }
    match query {
        None => Ok(servers
            .iter()
            .map(|s| format!("{} — {} tools", ident(s), tools_of(s).len()))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n\nCall search with `app` to see an app's tools."),
        Some(query) => {
            let words: Vec<String> = query.to_lowercase().split_whitespace().map(String::from).collect();
            let mut hits = Vec::new();
            for server in servers.iter().filter(|s| app.map_or(true, |a| **s == a || ident(s) == a)) {
                for (tool, about) in tools_of(server) {
                    let line = format!("tools.{}.{tool} — {about}", ident(server));
                    let lower = line.to_lowercase();
                    if words.iter().all(|w| lower.contains(w)) {
                        hits.push(line);
                    }
                }
            }
            if hits.is_empty() {
                Ok("No tools match. Try fewer words, or call search with no parameters to list the apps.".into())
            } else {
                Ok(hits.join("\n") + "\n\nCall search with `app` to see the full types.")
            }
        }
    }
}
