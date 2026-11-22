//! `dogu call` — run one tool. This is what every stub script ends up calling.

use crate::client::{Client, Options};
use crate::config;
use crate::sync::{safe, saved_era, saved_state, schema_path};
use serde_json::{json, Map, Value};
use std::io::Read;
use std::path::Path;
use std::time::Duration;

pub fn run(root: &Path, server: &str, tool: &str, args: &[String]) -> Result<i32, String> {
    // The schema was saved by `sync`, so --help and argument checks need no network.
    let schema: Value = std::fs::read_to_string(schema_path(root, server, tool))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", help(tool, &schema));
        return Ok(0);
    }
    let raw = args.iter().any(|a| a == "--raw");
    let as_json = args.iter().any(|a| a == "--json");
    let args: Vec<&String> = args.iter().filter(|a| *a != "--raw" && *a != "--json").collect();

    let input = &schema["inputSchema"];
    let arguments = parse_args(&args, input).map_err(|e| format!("{e}\nRun with --help to see the parameters."))?;

    // Catch missing parameters here: it is faster and the message is clearer.
    if let Some(required) = input["required"].as_array() {
        let missing: Vec<String> = required
            .iter()
            .filter_map(Value::as_str)
            .filter(|k| arguments.get(*k).is_none())
            .map(|k| format!("--{k}"))
            .collect();
        if !missing.is_empty() {
            return Err(format!("missing {}\nRun with --help to see the parameters.", missing.join(", ")));
        }
    }

    // A tool left out by `dogu sync --read-only` has no saved schema, and must not be callable either.
    if schema.is_null() && saved_state(root).is_some_and(|s| s["readOnly"] == true) {
        return Err(format!(
            "{server}/{tool} is not in this folder: it was synced with --read-only, which keeps only the tools \
             the server marks read-only. Run `dogu sync --all-tools` to bring the others back."
        ));
    }

    // A forged tool is a program, run in the sandbox.
    if server == crate::sync::FORGED {
        let started = std::time::Instant::now();
        let result = crate::forge::run(root, tool, &Value::Object(arguments), &mut crate::run::Pool::new());
        log_call(root, "call", server, tool, result.is_ok(), started);
        return match result {
            Ok(printed) => {
                print!("{printed}");
                Ok(0)
            }
            Err(e) => {
                eprintln!("{e}");
                Ok(1)
            }
        };
    }

    // Through the folder's background process, which keeps connections open; else directly.
    let started = std::time::Instant::now();
    let arguments = Value::Object(arguments);
    #[cfg(unix)]
    let warm = crate::warm::call(root, server, tool, &arguments);
    #[cfg(not(unix))]
    let warm = None;
    let result = match warm {
        Some(result) => result,
        None => call_directly(root, server, tool, arguments, input),
    };
    log_call(root, "call", server, tool, matches!(&result, Ok(r) if r["isError"] != true), started);
    let result = result?;

    if raw {
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    } else if as_json && result["isError"] != true {
        println!("{}", json_of(&result));
    } else {
        print_result(&result);
    }
    Ok(if result["isError"].as_bool() == Some(true) { 1 } else { 0 })
}

/// Find the server in the config, connect, and call it.
fn call_directly(root: &Path, server: &str, tool: &str, arguments: Value, input: &Value) -> Result<Value, String> {
    let source = config::Source::from_state(&saved_state(root).unwrap_or(Value::Null))?;
    let servers = config::load(&source)?;
    let spec = servers
        .get(server)
        .ok_or_else(|| format!("server \"{server}\" is not in {}. Run `dogu sync`.", source.describe()))?;
    let secs = std::env::var("DOGU_TIMEOUT").ok().and_then(|s| s.parse().ok()).unwrap_or(300);
    let mut options = Options::new(Duration::from_secs(secs));
    options.era = saved_era(root, server);
    let mut client = Client::connect(spec, &options)?;
    client.call_tool(tool, arguments, Some(input))
}

/// Turn command-line words into the tool's arguments. Three ways to pass them:
///   --name value ...     flags
///   '{"name": "value"}'  one JSON object
///   -                    one JSON object from stdin
fn parse_args(args: &[&String], input: &Value) -> Result<Map<String, Value>, String> {
    if args.is_empty() {
        return Ok(Map::new());
    }
    let first = args[0].as_str();
    if first == "-" || first.trim_start().starts_with('{') {
        let text = if first == "-" {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).map_err(|e| e.to_string())?;
            s
        } else {
            first.to_string()
        };
        return match serde_json::from_str(&text) {
            Ok(Value::Object(m)) => Ok(m),
            Ok(_) => Err("the JSON input must be an object: {\"name\": \"value\"}".into()),
            Err(e) => Err(format!("the JSON input is not valid: {e}")),
        };
    }

    let props = input["properties"].as_object();
    let open = input["additionalProperties"].as_bool() == Some(true) || props.map_or(true, |p| p.is_empty());
    let mut out = Map::new();
    let mut i = 0;
    while i < args.len() {
        let word = args[i].as_str();
        let Some(flag) = word.strip_prefix("--") else {
            return Err(format!("did not expect `{word}`. Pass parameters as --name value."));
        };
        i += 1;
        // --name=value or --name value
        let (key, mut value) = match flag.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (flag.to_string(), None),
        };
        let prop = props.and_then(|p| p.get(&key));
        if prop.is_none() && !open {
            let known: Vec<String> = props.unwrap().keys().map(|k| format!("--{k}")).collect();
            return Err(format!("unknown parameter --{key}. This tool takes: {}", known.join(", ")));
        }
        let kind = prop.map(kind_of).unwrap_or_default();
        if value.is_none() {
            let next = args.get(i).map(|s| s.as_str());
            if kind == "boolean" && !matches!(next, Some("true") | Some("false")) {
                value = Some("true".into()); // a bare --flag means true
            } else if let Some(n) = next {
                value = Some(n.to_string());
                i += 1;
            } else {
                return Err(format!("--{key} needs a value"));
            }
        }
        let value = value.unwrap();

        if kind == "array" {
            // --tag a --tag b   or   --tag '["a","b"]'
            let item_kind = prop.map(|p| kind_of(&p["items"])).unwrap_or_default();
            let list = out.entry(key.clone()).or_insert_with(|| json!([]));
            match serde_json::from_str::<Value>(&value) {
                Ok(Value::Array(items)) => list.as_array_mut().unwrap().extend(items),
                _ => list.as_array_mut().unwrap().push(convert(&key, &value, &item_kind)?),
            }
        } else {
            out.insert(key.clone(), convert(&key, &value, &kind)?);
        }
    }
    Ok(out)
}

/// The JSON type a schema asks for: "string", "integer", "array"... or "" if it does not say.
fn kind_of(schema: &Value) -> String {
    match &schema["type"] {
        Value::String(s) => s.clone(),
        Value::Array(list) => {
            list.iter().filter_map(Value::as_str).find(|t| *t != "null").unwrap_or("").to_string()
        }
        _ => String::new(),
    }
}

fn convert(key: &str, text: &str, kind: &str) -> Result<Value, String> {
    match kind {
        "string" => Ok(json!(text)),
        "integer" => text.parse::<i64>().map(|n| json!(n)).map_err(|_| format!("--{key} must be a whole number")),
        "number" => text.parse::<f64>().map(|n| json!(n)).map_err(|_| format!("--{key} must be a number")),
        "boolean" => text.parse::<bool>().map(|b| json!(b)).map_err(|_| format!("--{key} must be true or false")),
        "object" => serde_json::from_str(text).map_err(|_| format!("--{key} must be JSON")),
        // The schema does not say. Use JSON if it parses, text if not.
        _ => Ok(serde_json::from_str(text).unwrap_or_else(|_| json!(text))),
    }
}

fn print_result(result: &Value) {
    let text = result_text(result);
    if result["isError"].as_bool() == Some(true) {
        eprintln!("{text}");
    } else if !text.is_empty() || result["content"].as_array().map_or(false, |c| !c.is_empty()) {
        println!("{text}");
    }
}

/// The answer as one line of JSON, for `--json`: the structured result if the tool gave one, else its
/// text if that is JSON, else the text as a JSON string. Always valid JSON, so `jq` can read it.
fn json_of(result: &Value) -> String {
    if !result["structuredContent"].is_null() {
        return result["structuredContent"].to_string();
    }
    let text = result_text(result);
    serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)).to_string()
}

/// The answer from a tool as plain text.
pub fn result_text(result: &Value) -> String {
    let blocks = result["content"].as_array().cloned().unwrap_or_default();
    if blocks.is_empty() && !result["structuredContent"].is_null() {
        return serde_json::to_string_pretty(&result["structuredContent"]).unwrap();
    }
    let mut parts = Vec::new();
    for block in &blocks {
        parts.push(match block["type"].as_str() {
            Some("text") => block["text"].as_str().unwrap_or("").to_string(),
            // Pictures and sound go to a file. We say where it is.
            Some(kind @ ("image" | "audio")) => match save_media(block) {
                Ok(path) => format!("[{kind} saved to {path}]"),
                Err(e) => format!("[{kind} could not be saved: {e}]"),
            },
            Some("resource") => match block["resource"]["text"].as_str() {
                Some(text) => text.to_string(),
                None => format!("[resource {}]", block["resource"]["uri"].as_str().unwrap_or("?")),
            },
            Some("resource_link") => format!("[link {}]", block["uri"].as_str().unwrap_or("?")),
            _ => block.to_string(),
        });
    }
    parts.join("\n")
}

/// Keep a record of every tool call: when, which tool, did it work, how long it took.
/// Parameters and results are not recorded, because they can hold private data.
pub fn log_call(root: &Path, mode: &str, server: &str, tool: &str, ok: bool, started: std::time::Instant) {
    use std::io::Write;
    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = json!({ "time": when, "mode": mode, "server": server, "tool": tool, "ok": ok,
                       "ms": started.elapsed().as_millis() as u64 });
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(root.join(".dogu/log.jsonl")) {
        let _ = writeln!(f, "{line}");
    }
}

fn save_media(block: &Value) -> Result<String, String> {
    let bytes = crate::util::base64_decode(block["data"].as_str().ok_or("no data")?)?;
    let mime = block["mimeType"].as_str().unwrap_or("");
    let ext = safe(mime.rsplit('/').next().filter(|e| !e.is_empty()).unwrap_or("bin"));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("dogu-{stamp}.{ext}"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// The --help text, made from the saved schema.
fn help(tool: &str, schema: &Value) -> String {
    if schema.is_null() {
        return format!("{tool}\n\nNo saved description for this tool. Run `dogu sync` first.\n");
    }
    let name = safe(tool);
    let mut out = format!("{name}\n\n");
    if let Some(d) = schema["description"].as_str() {
        out.push_str(d.trim());
        out.push_str("\n\n");
    }
    if let Some(safety) = safety_of(schema) {
        out.push_str(&format!("The server says this tool is: {safety}.\n\n"));
    }

    let input = &schema["inputSchema"];
    let required: Vec<&str> =
        input["required"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let empty = Map::new();
    let props = input["properties"].as_object().unwrap_or(&empty);
    let keys = ordered_keys(props, &required);

    let usage = format!("  ./{name}{}", flags_of(schema));
    let json_line = format!("  ./{name} '{}'", json_example(&keys, &required, props).replace('\'', "'\\''"));
    let pipe_line = format!("  ... | ./{name} -");
    let width = json_line.chars().count().max(pipe_line.chars().count()) + 5;
    out.push_str(&format!("Usage:\n{usage}\n{json_line:width$}(same parameters as JSON)\n"));
    out.push_str(&format!("{pipe_line:width$}(JSON from a pipe)\n\n"));

    if keys.is_empty() {
        out.push_str("Parameters: none\n");
    } else {
        out.push_str("Parameters:\n");
        let width = keys.iter().map(|k| k.len()).max().unwrap_or(0);
        for k in &keys {
            let p = &props[*k];
            let mut line = format!("  --{k:width$}  {}", type_text(p));
            line.push_str(if required.contains(&k.as_str()) { ", required" } else { ", optional" });
            if !p["default"].is_null() {
                line.push_str(&format!(", default {}", p["default"]));
            }
            if let Some(d) = p["description"].as_str() {
                line.push_str(&format!("\n  {:width$}      {}", "", d.trim().replace('\n', " ")));
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    if let Some(fields) = returns_of(schema) {
        out.push_str(&format!("\nReturns (with --json): {fields}\n"));
    }
    out.push_str("\nAdd --json to print the result as one line of JSON, or --raw for the server's full answer.\n");
    out
}

/// What a tool may change, as its server says in its annotations. None when the server does not say:
/// dogu never guesses.
pub fn safety_of(tool: &Value) -> Option<&'static str> {
    let hints = &tool["annotations"];
    if hints["readOnlyHint"] == true {
        Some("read-only")
    } else if hints["destructiveHint"] == true {
        Some("may delete or overwrite")
    } else if hints["readOnlyHint"] == false {
        Some("changes things")
    } else {
        None
    }
}

/// How many returned fields `returns_of` names before it cuts the list.
const MAX_RETURN_FIELDS: usize = 8;

/// The top-level fields a tool returns, from its output schema: `{tag_name, name, html_url}`.
/// An agent that sees them can pick fields with jq straight away instead of looking at the output first.
pub fn returns_of(tool: &Value) -> Option<String> {
    shape_of(&tool["outputSchema"])
}

fn shape_of(schema: &Value) -> Option<String> {
    // `null | {...}` and other unions: use the first choice that has a shape.
    if let Some(choices) = ["anyOf", "oneOf"].iter().find_map(|k| schema[*k].as_array()) {
        return choices.iter().find_map(shape_of);
    }
    if let Some(props) = schema["properties"].as_object().filter(|p| !p.is_empty()) {
        let names: Vec<&str> = props.keys().map(String::as_str).take(MAX_RETURN_FIELDS).collect();
        let more = if props.len() > MAX_RETURN_FIELDS { ", …" } else { "" };
        return Some(format!("{{{}{more}}}", names.join(", ")));
    }
    if kind_of(schema) == "array" {
        return shape_of(&schema["items"]).map(|s| format!("[{s}]"));
    }
    None
}

/// Parameter names with the required ones first.
fn ordered_keys<'a>(props: &'a Map<String, Value>, required: &[&str]) -> Vec<&'a String> {
    let mut keys: Vec<&String> = props.keys().collect();
    keys.sort_by_key(|k| !required.contains(&k.as_str()));
    keys
}

/// A tool's flags as one line, required first: ` --query <string> [--sort stars|forks]`.
/// An enum shows its choices; any other type shows in angle brackets.
pub fn flags_of(schema: &Value) -> String {
    let input = &schema["inputSchema"];
    let required: Vec<&str> =
        input["required"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let Some(props) = input["properties"].as_object() else { return String::new() };
    ordered_keys(props, &required)
        .iter()
        .map(|k| {
            let value = match props[*k]["enum"].is_array() {
                true => type_text(&props[*k]),
                false => format!("<{}>", type_text(&props[*k])),
            };
            match required.contains(&k.as_str()) {
                true => format!(" --{k} {value}"),
                false => format!(" [--{k} {value}]"),
            }
        })
        .collect()
}

/// A JSON example for --help, from the tool's own parameters: the required ones, or else the
/// first one. Each value is the default, the first allowed value, or a stand-in of the right type.
fn json_example(keys: &[&String], required: &[&str], props: &Map<String, Value>) -> String {
    let required_keys: Vec<&&String> = keys.iter().filter(|k| required.contains(&k.as_str())).collect();
    let shown = if required_keys.is_empty() { keys.iter().take(1).collect() } else { required_keys };
    let pairs: Vec<String> =
        shown.iter().map(|k| format!("{}: {}", Value::String(k.to_string()), example_value(&props[k.as_str()]))).collect();
    format!("{{{}}}", pairs.join(", "))
}

fn example_value(p: &Value) -> Value {
    if !p["default"].is_null() {
        return p["default"].clone();
    }
    if let Some(first) = p["enum"].as_array().and_then(|o| o.first()) {
        return first.clone();
    }
    if let Some(first) = ["anyOf", "oneOf"].iter().find_map(|k| p[*k].as_array().and_then(|o| o.first())) {
        return example_value(first);
    }
    match kind_of(p).as_str() {
        "integer" | "number" => json!(1),
        "boolean" => json!(true),
        "array" => json!([]),
        "object" => json!({}),
        // A string, or a parameter whose type the schema does not say.
        _ => json!("..."),
    }
}

/// A short, readable type: "string", "integer", "a|b|c", "list of string"...
fn type_text(p: &Value) -> String {
    if let Some(options) = p["enum"].as_array() {
        let list: Vec<String> =
            options.iter().map(|o| o.as_str().map(String::from).unwrap_or_else(|| o.to_string())).collect();
        return list.join("|");
    }
    match kind_of(p).as_str() {
        "array" => format!("list of {}", type_text(&p["items"])),
        "object" => "json".into(),
        "" => "value".into(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(input: Value) -> Value {
        json!({ "name": "t", "description": "A tool.", "inputSchema": input })
    }

    #[test]
    fn help_json_example_uses_the_real_required_parameters() {
        let s = schema(json!({
            "type": "object",
            "properties": {
                "verbose": { "type": "boolean" },
                "path": { "type": "string" },
                "count": { "type": "integer" }
            },
            "required": ["path", "count"]
        }));

        let text = help("read_it", &s);

        assert!(text.contains(r#"./read_it '{"path": "...", "count": 1}'"#), "{text}");
        assert!(!text.contains(r#""name": "value""#), "{text}");
    }

    #[test]
    fn help_json_example_uses_the_first_parameter_when_none_is_required() {
        let s = schema(json!({
            "type": "object",
            "properties": {
                "orderBy": { "type": "string", "enum": ["createdAt", "updatedAt"] },
                "limit": { "type": "number", "default": 50 }
            }
        }));

        let text = help("list", &s);

        assert!(text.contains(r#"./list '{"orderBy": "createdAt"}'"#), "{text}");
    }

    #[test]
    fn help_json_example_fills_each_type_sensibly() {
        let s = schema(json!({
            "type": "object",
            "properties": {
                "a": { "type": "array", "items": { "type": "string" } },
                "b": { "type": "boolean" },
                "c": { "type": "object" },
                "d": { "type": "string", "default": "x" }
            },
            "required": ["a", "b", "c", "d"]
        }));

        let text = help("t", &s);

        assert!(text.contains(r#"'{"a": [], "b": true, "c": {}, "d": "x"}'"#), "{text}");
    }

    #[test]
    fn help_json_example_uses_the_first_choice_of_a_parameter_with_several_types() {
        let s = schema(json!({
            "type": "object",
            "properties": {
                "repoName": { "anyOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" } }] },
                "anything": {}
            },
            "required": ["repoName", "anything"]
        }));

        let text = help("ask", &s);

        assert!(text.contains(r#"'{"repoName": "...", "anything": "..."}'"#), "{text}");
    }

    #[test]
    fn help_json_example_is_an_empty_object_for_a_tool_with_no_parameters() {
        let text = help("ping", &schema(json!({ "type": "object", "properties": {} })));

        assert!(text.contains("./ping '{}'"), "{text}");
    }

    #[test]
    fn returns_lists_the_top_level_fields_of_the_output() {
        let tool = json!({ "outputSchema": { "type": "object", "properties": { "tag_name": {}, "name": {}, "html_url": {} } } });

        assert_eq!(returns_of(&tool).as_deref(), Some("{tag_name, name, html_url}"));
    }

    #[test]
    fn returns_sees_through_a_nullable_object_and_cuts_long_lists() {
        let fields: serde_json::Map<String, Value> = (1..=12).map(|i| (format!("f{i}"), json!({}))).collect();
        let tool = json!({ "outputSchema": { "anyOf": [{ "type": "null" }, { "type": "object", "properties": fields }] } });

        assert_eq!(returns_of(&tool).as_deref(), Some("{f1, f2, f3, f4, f5, f6, f7, f8, …}"));
    }

    #[test]
    fn returns_is_none_without_an_output_schema() {
        assert_eq!(returns_of(&json!({ "name": "t" })), None);
    }

    #[test]
    fn help_shows_what_a_tool_returns() {
        let s = json!({ "name": "t", "inputSchema": { "type": "object", "properties": {} },
                        "outputSchema": { "type": "object", "properties": { "tag_name": {}, "name": {} } } });

        assert!(help("t", &s).contains("Returns (with --json): {tag_name, name}"), "{}", help("t", &s));
    }

    #[test]
    fn help_usage_lines_stay_aligned() {
        let s = schema(json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        }));

        let text = help("read_it", &s);
        let json_line = text.lines().find(|l| l.contains("(same parameters as JSON)")).unwrap();
        let pipe_line = text.lines().find(|l| l.contains("(JSON from a pipe)")).unwrap();

        assert_eq!(json_line.find('('), pipe_line.find("(JSON"), "{text}");
    }
}
