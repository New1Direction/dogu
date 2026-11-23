//! `dogu import`: copy your MCP servers into dogu's own config, `~/.config/dogu/mcp.json`, so the app
//! you use (Claude Code, Claude Desktop, Cursor) can stop loading them itself and use dogu instead.
//! Without this, removing a server from the app would also remove it from dogu, which reads the
//! same file.
//!
//! It copies each server as written, so `${VAR}` stays unexpanded, and gives each local server the
//! folder it ran in. It never changes the app's config and never replaces a server already in
//! dogu's. The file can hold tokens, so it is written readable only by you.

use crate::config::{self, Source};
use crate::sync;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// Where a server came from in Claude Code's config, for the `claude mcp remove` line.
#[derive(Clone, Copy)]
enum Scope {
    User,
    Local,
    /// Any other app's config: it has no scopes.
    Shared,
}

struct Found {
    name: String,
    entry: Value,
    scope: Scope,
}

pub fn target() -> PathBuf {
    config::home().join(".config/dogu/mcp.json")
}

pub fn run(source: &Source, out: &Path, read_only: Option<bool>) -> Result<(), String> {
    let target = target();
    if same_file(&source.path, &target) {
        println!("Your servers are already in {}. Nothing to import.", config::tilde(&target));
        return Ok(());
    }
    let found = servers_in(source)?;
    if found.is_empty() {
        return Err(format!("{} lists no servers to import", source.describe()));
    }
    let (added, kept) = merge_into(&target, &found)?;
    println!("Copied {} server(s) from {} to {}: {}", added.len(), source.describe(), config::tilde(&target), list(&added));
    if !kept.is_empty() {
        println!("Already there, kept yours: {}", list(&kept));
    }
    println!();
    let ours = config::find(Some(&target.to_string_lossy()))?;
    sync::run(&ours, out, read_only)?;
    println!();
    print_switch(source, &found);
    Ok(())
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn list(names: &[String]) -> String {
    if names.is_empty() { "none".into() } else { names.join(", ") }
}

/// The servers in the source, as written, each with the folder it runs in.
fn servers_in(source: &Source) -> Result<Vec<Found>, String> {
    let text = std::fs::read_to_string(&source.path).map_err(|e| format!("cannot read {}: {e}", source.path.display()))?;
    let json: Value = serde_json::from_str(&text).map_err(|e| format!("{} is not valid JSON: {e}", source.path.display()))?;
    let is_claude_code = json.get("projects").is_some() || source.path.file_name().is_some_and(|f| f == ".claude.json");
    let config_dir = std::fs::canonicalize(source.path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")))
        .map_err(|e| format!("cannot find the folder of {}: {e}", source.path.display()))?;
    let shared = json.get("mcpServers").or_else(|| json.get("servers"));
    let shared_scope = if is_claude_code { Scope::User } else { Scope::Shared };
    let mut found = entries(shared, &config_dir, shared_scope);
    if let Some(project) = &source.project {
        let local = entries(json["projects"][project].get("mcpServers"), Path::new(project), Scope::Local);
        // A project's own server wins over the user's of the same name, as in Claude Code.
        found.retain(|f| !local.iter().any(|l| l.name == f.name));
        found.extend(local);
    }
    Ok(found)
}

fn entries(servers: Option<&Value>, run_in: &Path, scope: Scope) -> Vec<Found> {
    let Some(servers) = servers.and_then(Value::as_object) else { return Vec::new() };
    servers
        .iter()
        .filter(|(_, entry)| entry.is_object() && !is_dogu_serve(entry))
        .map(|(name, entry)| {
            let mut entry = entry.clone();
            // A local server ran in the config's folder (or the project's): keep it there.
            if entry.get("command").is_some() && entry.get("cwd").is_none() {
                entry["cwd"] = json!(run_in.to_string_lossy());
            }
            Found { name: name.clone(), entry, scope }
        })
        .collect()
}

fn is_dogu_serve(entry: &Value) -> bool {
    let command = entry["command"].as_str().unwrap_or("");
    let serves = entry["args"].as_array().is_some_and(|a| a.iter().any(|x| x == "serve"));
    Path::new(command).file_name().is_some_and(|f| f == "dogu") && serves
}

/// Adds the servers to the target, keeping any it already has. Returns (added, kept).
fn merge_into(target: &Path, found: &[Found]) -> Result<(Vec<String>, Vec<String>), String> {
    let mut doc: Value = match std::fs::read_to_string(target) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{} is not valid JSON: {e}", target.display()))?,
        Err(_) => json!({ "mcpServers": {} }),
    };
    if !doc["mcpServers"].is_object() {
        doc["mcpServers"] = Value::Object(Map::new());
    }
    let servers = doc["mcpServers"].as_object_mut().expect("an object, set just above");
    let (mut added, mut kept) = (Vec::new(), Vec::new());
    for f in found {
        if servers.contains_key(&f.name) {
            kept.push(f.name.clone());
        } else {
            servers.insert(f.name.clone(), f.entry.clone());
            added.push(f.name.clone());
        }
    }
    write_private(target, &serde_json::to_string_pretty(&doc).expect("JSON values serialize"))?;
    Ok((added, kept))
}

/// Writes the file readable only by you (it can hold tokens), replacing it in one step.
fn write_private(path: &Path, text: &str) -> Result<(), String> {
    let dir = path.parent().ok_or("the config has no folder")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let tmp = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&tmp).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::io::Write::write_all(&mut file, format!("{text}\n").as_bytes()).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    #[cfg(unix)]
    std::fs::set_permissions(&tmp, std::os::unix::fs::PermissionsExt::from_mode(0o600))
        .map_err(|e| format!("cannot set permissions on {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// What to run so the app stops loading the servers itself and uses dogu.
fn print_switch(source: &Source, found: &[Found]) {
    let is_claude_code = found.iter().any(|f| !matches!(f.scope, Scope::Shared));
    if is_claude_code {
        println!("Now switch Claude Code to dogu's code mode, so it stops loading these servers itself:");
        println!("  claude mcp add --scope user tools -- dogu serve");
        for f in found {
            let scope = if matches!(f.scope, Scope::Local) { "local" } else { "user" };
            println!("  claude mcp remove {} --scope {scope}", f.name);
        }
        println!("dogu keeps working: it reads {} from now on.", config::tilde(&target()));
    } else {
        println!("Now, in the app that uses {}, replace its servers with this one:", config::tilde(&source.path));
        println!("  {}", json!({ "mcpServers": { "tools": { "command": "dogu", "args": ["serve"] } } }));
    }
}
