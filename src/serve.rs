//! `dogu serve` — code mode for apps that have no shell (Claude Desktop and others).
//!
//! We act as one MCP server with two tools. All your real servers hide behind it:
//!   search    find tools and read their types
//!   run_code  run a TypeScript program in the sandbox

use crate::run::{ident, run_program, Options, Pool};
use crate::sync::{safe, tool_decl};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const MODERN: &str = crate::client::MODERN;
/// The MCP versions we answer to: the modern one first, then the legacy ones.
const VERSIONS: [&str; 4] = [MODERN, "2025-11-25", "2025-06-18", "2025-03-26"];

const INSTRUCTIONS: &str = "\
This server gives you many tools through a few. The description of `search` lists every app and tool. \
Call `search` with the words of the tool you need to read its TypeScript type. Then call `run_code` with \
one TypeScript program that calls it as `await tools.<app>.<tool>({...})`. Do several steps in one \
program, and print only what you need to see. When a program is worth keeping, save it with `forge` and \
call it next time as `tools.forged.<name>({...})`.";

/// How many tools one search returns, each with its full type.
const MAX_HITS: usize = 6;
/// The list of every app and tool in `search`'s description stays under this size; past it, apps only.
const CATALOG_BUDGET: usize = 8_000;

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
                    "description": format!("Find tools and read their TypeScript types. With `query`: the best \
                                    matching tools with their types; put several tool names in one query to get \
                                    all their types in one call. With `app`: all that app's types. With nothing: \
                                    the apps.\n\n{}", catalog(opts)),
                    "inputSchema": { "type": "object", "properties": {
                        "app": { "type": "string", "description": "An app name from the list" },
                        "query": { "type": "string", "description": "Words to look for" }
                    } }
                },
                {
                    "name": "forge",
                    "description": "Save a program as a new tool you can call later as `tools.forged.<name>({...})`, \
                                    in this session or another. Start the code with `// @description <what it does>` \
                                    and a `// @param <name> <type> [required] <what it is>` line per input (types: \
                                    string, number, integer, boolean, json); inside, the inputs are in `args`. Print \
                                    the result, as JSON if you can. It may only call the tools its code names.",
                    "inputSchema": { "type": "object", "required": ["name", "code"], "properties": {
                        "name": { "type": "string", "description": "lowercase_with_underscores" },
                        "code": { "type": "string", "description": "The program, header first" },
                        "replace": { "type": "boolean", "description": "Replace a forged tool of the same name" }
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
                    Some("forge") => forge_tool(opts, args),
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

/// An app's tools as (name in code, schema), sorted by name.
fn tools_of(opts: &Options, server: &str) -> Vec<(String, Value)> {
    let dir = opts.root.join(".dogu/schemas").join(safe(server));
    let mut list: Vec<(String, Value)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
        .map(|schema| (ident(schema["name"].as_str().unwrap_or("")), schema))
        .collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    list
}

/// The synced apps, then `forged` if agents have made any tools.
fn servers_of(opts: &Options) -> Vec<String> {
    let mut servers: Vec<String> = crate::sync::saved_state(&opts.root)
        .and_then(|state| state["servers"].as_array().cloned())
        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default();
    if !crate::sync::saved_schemas(&opts.root, crate::sync::FORGED).is_empty() {
        servers.push(crate::sync::FORGED.to_string());
    }
    servers
}

/// The `forge` tool: save a program as `tools.forged.<name>`.
fn forge_tool(opts: &Options, args: &Value) -> Result<String, String> {
    let name = args["name"].as_str().ok_or("forge needs a `name`")?;
    let code = args["code"].as_str().ok_or("forge needs the `code`")?;
    crate::forge::save(&opts.root, name, code, args["replace"].as_bool() == Some(true))
}

/// Every app and its tool names, for `search`'s description, so the model knows what to ask for.
fn catalog(opts: &Options) -> String {
    let servers = servers_of(opts);
    let named: Vec<String> = servers
        .iter()
        .map(|s| format!("{}: {}", ident(s), tools_of(opts, s).into_iter().map(|(n, _)| n).collect::<Vec<_>>().join(", ")))
        .collect();
    let full = format!("Apps and their tools:\n{}", named.join("\n"));
    if full.len() <= CATALOG_BUDGET {
        return full;
    }
    let counts: Vec<String> = servers.iter().map(|s| format!("{} ({} tools)", ident(s), tools_of(opts, s).len())).collect();
    format!("Apps: {}", counts.join(", "))
}

fn search(opts: &Options, app: Option<&str>, query: Option<&str>) -> Result<String, String> {
    let servers = servers_of(opts);
    if let Some(app) = app {
        let server = servers
            .iter()
            .find(|s| *s == app || ident(s) == app)
            .ok_or_else(|| format!("no app \"{app}\". Apps: {}", servers.iter().map(|s| ident(s)).collect::<Vec<_>>().join(", ")))?;
        if query.is_none() {
            return std::fs::read_to_string(opts.root.join(safe(server)).join("index.d.ts")).map_err(|e| e.to_string());
        }
    }
    let Some(query) = query else {
        return Ok(catalog(opts) + "\n\nCall search with words from a tool's name or job to read its type.");
    };
    // Rank by how many of the words a tool's app, name and description contain. All words are not needed.
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(String::from).collect();
    let mut hits: Vec<(usize, String, Value)> = Vec::new();
    for server in servers.iter().filter(|s| app.map_or(true, |a| *s == a || ident(s) == a)) {
        for (tool, schema) in tools_of(opts, server) {
            let about = schema["description"].as_str().unwrap_or("");
            // The name both as written and with `_` as spaces, so `search_repositories` and `search repositories` both match.
            let text = format!("{} {tool} {} {about}", ident(server), tool.replace('_', " ")).to_lowercase();
            let score = words.iter().filter(|w| text.contains(w.as_str())).count();
            if score > 0 {
                hits.push((score, ident(server), schema));
            }
        }
    }
    if hits.is_empty() {
        return Ok(format!("No tools match. {}", catalog(opts)));
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0));
    let shown: Vec<String> =
        hits.iter().take(MAX_HITS).map(|(_, app, schema)| tool_decl(schema, "", &format!("tools.{app}."))).collect();
    let more = match hits.len() > MAX_HITS {
        true => format!("\n({} more matched. Add words to narrow it.)", hits.len() - MAX_HITS),
        false => String::new(),
    };
    Ok(format!("{}{more}\nCall run_code with a program that uses them.", shown.join("")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A folder like `dogu sync` makes, with one app "fx" and two tools.
    fn folder(name: &str) -> Options {
        let root = std::env::temp_dir().join(format!("dogu-serve-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let schemas = root.join(".dogu/schemas/fx");
        std::fs::create_dir_all(&schemas).unwrap();
        std::fs::write(root.join(".dogu/state.json"), json!({ "servers": ["fx"] }).to_string()).unwrap();
        let search = json!({ "name": "search_repositories", "description": "Search GitHub repositories by a query.",
            "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] } });
        let count = json!({ "name": "count", "description": "Add one to a counter.", "inputSchema": { "type": "object" } });
        std::fs::write(schemas.join("search_repositories.json"), search.to_string()).unwrap();
        std::fs::write(schemas.join("count.json"), count.to_string()).unwrap();
        Options::new(root)
    }

    #[test]
    fn search_finds_a_tool_when_only_some_words_match() {
        let opts = folder("some-words");

        let found = search(&opts, None, Some("github repositories org")).unwrap();

        assert!(found.contains("search_repositories"), "{found}");
        assert!(!found.contains("No tools match"), "{found}");
    }

    #[test]
    fn search_results_include_the_types_so_no_second_search_is_needed() {
        let opts = folder("types");

        let found = search(&opts, None, Some("repositories")).unwrap();

        assert!(found.contains("tools.fx.search_repositories(args: {"), "{found}");
        assert!(found.contains("query: string"), "{found}");
    }

    #[test]
    fn search_finds_tools_by_their_exact_names() {
        let opts = folder("exact-names");

        let found = search(&opts, None, Some("search_repositories count")).unwrap();

        assert!(found.contains("tools.fx.search_repositories(args: {"), "{found}");
        assert!(found.contains("tools.fx.count("), "{found}");
    }

    #[test]
    fn search_ranks_the_best_match_first() {
        let opts = folder("rank");

        let found = search(&opts, None, Some("counter add")).unwrap();

        let count_at = found.find("tools.fx.count").expect("count is listed");
        assert!(found.find("tools.fx.search_repositories").map_or(true, |at| at > count_at), "{found}");
    }

    #[test]
    fn the_forge_tool_saves_a_tool_that_search_then_finds() {
        let opts = folder("forge");
        let code = "// @description Count up once.\nconsole.log(JSON.stringify(await tools.fx.count({})));\n";

        let saved = forge_tool(&opts, &json!({ "name": "count_once", "code": code })).unwrap();

        assert!(saved.contains("Saved forged/count_once"), "{saved}");
        assert!(catalog(&opts).contains("forged: count_once"), "{}", catalog(&opts));
        assert!(search(&opts, None, Some("count once")).unwrap().contains("tools.forged.count_once("));
    }

    #[test]
    fn the_tool_list_names_every_app_and_tool() {
        let opts = folder("catalog");

        let text = catalog(&opts);

        assert!(text.contains("fx: count, search_repositories"), "{text}");
    }
}
