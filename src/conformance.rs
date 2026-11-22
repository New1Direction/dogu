//! A hidden command for the official MCP conformance suite (@modelcontextprotocol/conformance).
//! The suite starts a test server and runs:  dogu conformance <server-url>
//! We connect, list the tools and call each one, and the suite checks what it saw on the wire.

use crate::client::{Client, Options};
use crate::config::Server;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::time::Duration;

pub fn run(url: &str) -> Result<i32, String> {
    let context: Value = std::env::var("MCP_CONFORMANCE_CONTEXT")
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    // The suite's login pages need no person, so we follow them ourselves.
    std::env::set_var("DOGU_AUTH_HEADLESS", "1");
    if std::env::var("DOGU_AUTH_FILE").is_err() {
        let file = std::env::temp_dir().join(format!("dogu-conformance-{}.json", std::process::id()));
        std::env::set_var("DOGU_AUTH_FILE", &file);
    }
    if std::env::var("DOGU_CLIENT_ID_URL").is_err() {
        std::env::set_var("DOGU_CLIENT_ID_URL", "https://conformance-test.local/client-metadata.json");
    }
    let mut oauth = Map::new();
    for key in ["client_id", "client_secret"] {
        if let Some(v) = context[key].as_str() {
            oauth.insert(key.into(), json!(v));
        }
    }
    let server = Server::Http {
        name: "conformance".into(),
        url: url.to_string(),
        headers: BTreeMap::new(),
        oauth: Value::Object(oauth),
    };
    let mut options = Options::new(Duration::from_secs(20));
    options.interactive = true;

    let result = (|| -> Result<(), String> {
        let mut client = Client::connect(&server, &options)?;
        let tools = client.list_tools()?;
        eprintln!("era: {:?}, tools: {}", client.era(), tools.len());
        // Some scenarios list the exact calls to make.
        if let Some(calls) = context["toolCalls"].as_array() {
            for call in calls {
                let name = call["name"].as_str().unwrap_or("");
                let Some(tool) = tools.iter().find(|t| t["name"] == name) else { continue };
                let result = client.call_tool(name, call["arguments"].clone(), Some(&tool["inputSchema"]));
                eprintln!("call {name} -> {}", result.map(|r| r.to_string()).unwrap_or_else(|e| e).chars().take(200).collect::<String>());
            }
            return Ok(());
        }
        for tool in &tools {
            let name = tool["name"].as_str().unwrap_or("");
            if let Err(why) = crate::client::header_params(&tool["inputSchema"]) {
                eprintln!("skip {name}: {why}");
                continue;
            }
            let mut args = example_arguments(&tool["inputSchema"], &context);
            // One scenario checks that a tool's schema comes through us unchanged: it asks for the
            // schema of another tool to be sent back as an argument.
            if name == "json_schema_echo" {
                if let Some(other) = tools.iter().find(|t| t["name"] == "json_schema_2020_12_tool") {
                    args = json!({ "schema": other["inputSchema"] });
                }
            }
            match client.call_tool(name, args.clone(), Some(&tool["inputSchema"])) {
                Ok(r) => eprintln!("call {name} {args} -> {}", r.to_string().chars().take(200).collect::<String>()),
                Err(e) => eprintln!("call {name} {args} failed: {e}"),
            }
        }
        Ok(())
    })();
    if let Ok(file) = std::env::var("DOGU_AUTH_FILE") {
        if file.contains("dogu-conformance-") {
            let _ = std::fs::remove_file(file);
        }
    }
    match result {
        Ok(()) => Ok(0),
        Err(e) => {
            eprintln!("error: {e}");
            Ok(1)
        }
    }
}

/// Make up arguments that fit a tool's schema.
fn example_arguments(schema: &Value, context: &Value) -> Value {
    // Some scenarios say which arguments to send.
    if let Some(given) = context.get("toolArguments").or(context.get("arguments")).filter(|v| v.is_object()) {
        return given.clone();
    }
    fn example(schema: &Value, name: &str, index: usize) -> Value {
        if let Some(first) = schema["enum"].as_array().and_then(|e| e.first()) {
            return first.clone();
        }
        if !schema["default"].is_null() {
            return schema["default"].clone();
        }
        match schema["type"].as_str() {
            Some("integer") | Some("number") => json!([5, 3, 2, 1][index.min(3)]),
            Some("boolean") => json!(true),
            Some("array") => json!([]),
            Some("object") => {
                let mut out = Map::new();
                for (i, (key, sub)) in schema["properties"].as_object().into_iter().flatten().enumerate() {
                    out.insert(key.clone(), example(sub, key, i));
                }
                Value::Object(out)
            }
            _ => json!(format!("test-{name}")),
        }
    }
    example(schema, "", 0)
}
