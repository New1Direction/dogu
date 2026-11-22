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
This server gives you many tools through a few. The description of `run_code` lists every tool with its \
arguments and the fields it returns, so you can usually call `run_code` straight away with one TypeScript \
program that calls them as `await tools.<app>.<tool>({...})`. Use `search` only when you need a tool's full \
type. Do several steps in one program, and print only what you need to see. When a program is worth \
keeping, save it with `forge` and call it next time as `tools.forged.<name>({...})`.";

/// The tool list in `run_code`'s description stays under this size (about 4k tokens): one line per tool
/// with its arguments, returns and a short description; then without descriptions; then names only.
const SIGNATURE_BUDGET: usize = 16_000;

/// What run_code may hand back to the model. Past this, the middle is cut out with a note, so one
/// careless print cannot flood the conversation.
const MAX_RUN_OUTPUT: usize = 30_000;

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
                    "description": "Read tools' full TypeScript types and descriptions, when the list in run_code's \
                                    description is not enough. With `query`: the best matching tools; put several \
                                    tool names in one query to get them all at once. With `app`: all that app's \
                                    types. With nothing: the apps.",
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
                    "description": format!("Run a TypeScript program in a sandbox. It cannot use files or the network. \
                                    It can call tools with `await tools.<app>.<tool>({{...}})` and print with \
                                    console.log; independent calls can run together with Promise.all. Returns what \
                                    the program printed.\n\nThe tools (arguments → the fields they return):\n{}",
                                    signatures(opts, SIGNATURE_BUDGET)),
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
    let text = cap_output(text);
    if outcome.code == 0 {
        Ok(if text.is_empty() { "(the program printed nothing)".into() } else { text })
    } else {
        Err(text)
    }
}

/// Keep the start and the end of a long output, and say how much was cut from the middle.
fn cap_output(text: String) -> String {
    if text.len() <= MAX_RUN_OUTPUT {
        return text;
    }
    let boundary = |mut at: usize| {
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        at
    };
    let head = boundary(MAX_RUN_OUTPUT * 2 / 3);
    let tail = boundary(text.len() - MAX_RUN_OUTPUT / 3);
    format!(
        "{}\n…{} characters cut from the middle. Print less, or filter in the program.…\n{}",
        &text[..head],
        tail - head,
        &text[tail..]
    )
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

/// A tool's arguments as a short TypeScript object type: `{ q: string, sort?: "stars"|"forks" }`.
fn compact_args(schema: &Value) -> String {
    let input = &schema["inputSchema"];
    let required: Vec<&str> = input["required"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let Some(props) = input["properties"].as_object().filter(|p| !p.is_empty()) else { return "{}".into() };
    let fields: Vec<String> = props
        .iter()
        .map(|(k, p)| format!("{k}{}: {}", if required.contains(&k.as_str()) { "" } else { "?" }, compact_type(p)))
        .collect();
    format!("{{ {} }}", fields.join(", "))
}

fn compact_type(p: &Value) -> String {
    if let Some(choices) = p["enum"].as_array() {
        let shown: Vec<String> = choices.iter().take(4).map(|c| c.to_string()).collect();
        return if choices.len() > 4 { format!("{}|…", shown.join("|")) } else { shown.join("|") };
    }
    if let Some(first) = ["anyOf", "oneOf"].iter().find_map(|k| p[*k].as_array().and_then(|a| a.first())) {
        return compact_type(first);
    }
    let kind = match &p["type"] {
        Value::String(s) => s.as_str(),
        Value::Array(list) => list.iter().filter_map(Value::as_str).find(|t| *t != "null").unwrap_or(""),
        _ => "",
    };
    match kind {
        "string" => "string".into(),
        "number" | "integer" => "number".into(),
        "boolean" => "boolean".into(),
        "array" => format!("{}[]", compact_type(&p["items"])),
        "object" => "object".into(),
        _ => "any".into(),
    }
}

/// Every tool, one line each, for `run_code`'s description, so the model can usually write its program
/// without a search first. Within `budget`: with descriptions, then without, then the names only.
fn signatures(opts: &Options, budget: usize) -> String {
    let servers = servers_of(opts);
    let render = |about: bool| -> String {
        servers
            .iter()
            .flat_map(|s| tools_of(opts, s).into_iter().map(move |(name, schema)| (ident(s), name, schema)))
            .map(|(app, name, schema)| {
                let returns = crate::call::returns_of(&schema).map(|r| format!(" → {r}")).unwrap_or_default();
                let note = match about {
                    true => schema["description"]
                        .as_str()
                        .map(|d| d.trim().split(['\n', '.']).next().unwrap_or("").chars().take(90).collect::<String>())
                        .filter(|d| !d.is_empty())
                        .map(|d| format!(" — {d}"))
                        .unwrap_or_default(),
                    false => String::new(),
                };
                format!("tools.{app}.{name}({}){returns}{note}", compact_args(&schema))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    [true, false].into_iter().map(render).find(|t| t.len() <= budget).unwrap_or_else(|| catalog(opts))
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
    fn a_huge_program_output_is_cut_in_the_middle_with_a_note() {
        let text = format!("start{}end", "x".repeat(MAX_RUN_OUTPUT * 2));

        let cut = cap_output(text);

        assert!(cut.len() < MAX_RUN_OUTPUT + 200, "{}", cut.len());
        assert!(cut.starts_with("start") && cut.ends_with("end"));
        assert!(cut.contains("characters cut from the middle"), "{}", &cut[..200.min(cut.len())]);
    }

    #[test]
    fn a_normal_program_output_is_left_alone() {
        assert_eq!(cap_output("hello\n".to_string()), "hello\n");
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
    fn run_code_lists_every_tool_with_its_arguments_so_no_search_is_needed() {
        let opts = folder("signatures");

        let text = signatures(&opts, SIGNATURE_BUDGET);

        assert!(text.contains("tools.fx.search_repositories({ query: string })"), "{text}");
        assert!(text.contains("tools.fx.count({})"), "{text}");
        assert!(text.contains("Search GitHub repositories by a query"), "{text}");
    }

    #[test]
    fn a_long_tool_list_drops_descriptions_then_falls_back_to_names() {
        let opts = folder("signature-budget");
        let full = signatures(&opts, SIGNATURE_BUDGET);

        let short = signatures(&opts, full.len() - 1);
        let names = signatures(&opts, 10);

        assert!(short.contains("tools.fx.search_repositories({ query: string })") && !short.contains("Search GitHub"), "{short}");
        assert!(names.contains("fx: count, search_repositories"), "{names}");
    }

    #[test]
    fn compact_types_read_like_typescript() {
        let schema = json!({ "inputSchema": { "type": "object", "required": ["q"], "properties": {
            "q": { "type": "string" },
            "sort": { "type": "string", "enum": ["stars", "forks"] },
            "ids": { "type": "array", "items": { "type": "integer" } },
            "opts": { "type": "object" },
            "flag": { "type": "boolean" }
        } } });

        assert_eq!(compact_args(&schema), r#"{ q: string, sort?: "stars"|"forks", ids?: number[], opts?: object, flag?: boolean }"#);
    }
}
