//! Reads the same `mcp.json` that Claude Desktop, Claude Code, Cursor and others use.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub enum Server {
    /// A local program we start and talk to over stdin/stdout.
    /// It runs in `cwd`: the config's "cwd", or else the folder the config file is in,
    /// so relative paths in the config work no matter where a tool is called from.
    Stdio { command: String, args: Vec<String>, env: BTreeMap<String, String>, cwd: PathBuf },
    /// A remote server we talk to over HTTP. `oauth` may hold a client_id you registered yourself.
    Http { name: String, url: String, headers: BTreeMap<String, String>, oauth: Value },
}

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

/// Find the config file: --config, then $DOGU_CONFIG, then the usual places.
pub fn find(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        return Ok(PathBuf::from(p));
    }
    if let Ok(p) = std::env::var("DOGU_CONFIG") {
        return Ok(PathBuf::from(p));
    }
    let h = home();
    let places = [
        PathBuf::from(".mcp.json"),
        PathBuf::from("mcp.json"),
        h.join(".config/dogu/mcp.json"),
        h.join("Library/Application Support/Claude/claude_desktop_config.json"),
        h.join(".config/Claude/claude_desktop_config.json"),
        h.join(".cursor/mcp.json"),
    ];
    for p in &places {
        if p.is_file() {
            return Ok(p.clone());
        }
    }
    let tried: Vec<String> = places.iter().map(|p| format!("  {}", p.display())).collect();
    Err(format!("no MCP config found. Pass --config <file>. Looked in:\n{}", tried.join("\n")))
}

pub fn load(path: &Path) -> Result<BTreeMap<String, Server>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let json: Value = serde_json::from_str(&text).map_err(|e| format!("{} is not valid JSON: {e}", path.display()))?;
    let servers = json
        .get("mcpServers")
        .or_else(|| json.get("servers"))
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{} has no \"mcpServers\" object", path.display()))?;

    let mut out = BTreeMap::new();
    for (name, s) in servers {
        if s.get("disabled").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let server = if let Some(url) = s.get("url").and_then(Value::as_str) {
            Server::Http {
                name: name.clone(),
                url: expand(url),
                headers: string_map(s.get("headers")),
                oauth: s.get("oauth").cloned().unwrap_or(Value::Null),
            }
        } else if let Some(command) = s.get("command").and_then(Value::as_str) {
            let args = s
                .get("args")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(expand).collect())
                .unwrap_or_default();
            let cwd = match s.get("cwd").and_then(Value::as_str) {
                Some(dir) => PathBuf::from(expand(dir)),
                None => path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")).to_path_buf(),
            };
            Server::Stdio { command: expand(command), args, env: string_map(s.get("env")), cwd }
        } else {
            return Err(format!("server \"{name}\" needs a \"command\" or a \"url\""));
        };
        out.insert(name.clone(), server);
    }
    Ok(out)
}

fn string_map(v: Option<&Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(obj) = v.and_then(Value::as_object) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                out.insert(k.clone(), expand(s));
            }
        }
    }
    out
}

/// Replace `${NAME}` with the environment variable NAME, so secrets can stay out of the file.
fn expand(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find('}') {
            Some(end) => {
                let name = &rest[start + 2..start + 2 + end];
                out.push_str(&std::env::var(name).unwrap_or_default());
                rest = &rest[start + 2 + end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}
