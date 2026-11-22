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

/// Where the servers come from: a config file and, for Claude Code's `~/.claude.json`, the
/// project folder whose own servers are added. `claude mcp add` saves to that "local" scope by
/// default, under `projects.<folder>.mcpServers`.
pub struct Source {
    pub path: PathBuf,
    pub project: Option<String>,
}

impl Source {
    /// A config file, with the servers for the current folder if it is Claude Code's.
    fn at(path: PathBuf) -> Source {
        let project = read_json(&path).ok().and_then(|json| project_for_cwd(&json));
        Source { path, project }
    }

    /// The source a folder was made from, as `dogu sync` saved it in `.dogu/state.json`.
    pub fn from_state(state: &Value) -> Result<Source, String> {
        match state["config"].as_str() {
            Some(path) => Ok(Source {
                path: PathBuf::from(path),
                project: state["project"].as_str().map(String::from),
            }),
            None => find(None),
        }
    }

    pub fn describe(&self) -> String {
        match &self.project {
            Some(project) => format!("{}, with the servers for {project}", self.path.display()),
            None => self.path.display().to_string(),
        }
    }
}

/// Find the config: --config, then $DOGU_CONFIG, then the first of the usual places that lists
/// any servers.
pub fn find(explicit: Option<&str>) -> Result<Source, String> {
    if let Some(p) = explicit.map(String::from).or_else(|| std::env::var("DOGU_CONFIG").ok()) {
        return Ok(Source::at(PathBuf::from(p)));
    }
    let h = home();
    let places = [
        PathBuf::from(".mcp.json"),
        PathBuf::from("mcp.json"),
        h.join(".config/dogu/mcp.json"),
        h.join(".claude.json"),
        h.join("Library/Application Support/Claude/claude_desktop_config.json"),
        h.join(".config/Claude/claude_desktop_config.json"),
        h.join(".cursor/mcp.json"),
    ];
    let claude_code = h.join(".claude.json");
    for p in places.iter().filter(|p| p.is_file()) {
        let source = Source::at(p.clone());
        // Claude Code writes ~/.claude.json even when you have no servers, so skip it then.
        // Any other file is yours: use it, so an empty or broken one shows its error.
        if *p != claude_code || !matches!(load(&source), Ok(servers) if servers.is_empty()) {
            return Ok(source);
        }
    }
    let tried: Vec<String> = places.iter().map(|p| format!("  {}", p.display())).collect();
    Err(format!("no MCP config found. Pass --config <file>. Looked in:\n{}", tried.join("\n")))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} is not valid JSON: {e}", path.display()))
}

/// The current folder, if Claude Code's config lists servers for it.
fn project_for_cwd(json: &Value) -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let key = cwd.to_str()?;
    let servers = json["projects"][key]["mcpServers"].as_object()?;
    (!servers.is_empty()).then(|| key.to_string())
}

/// All servers in the source. A project's own servers win over the user's, as in Claude Code.
pub fn load(source: &Source) -> Result<BTreeMap<String, Server>, String> {
    let path = &source.path;
    let json = read_json(path)?;
    let config_dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let shared = json.get("mcpServers").or_else(|| json.get("servers"));
    // Claude Code's file has no "mcpServers" until you add one. Any other config needs it.
    let is_claude_code = json.get("projects").is_some() || path.file_name().is_some_and(|f| f == ".claude.json");
    if shared.is_none() && !is_claude_code {
        return Err(format!("{} has no \"mcpServers\" object", path.display()));
    }
    let mut out = parse_servers(shared, config_dir).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(project) = &source.project {
        // A project's local servers run in the project folder, as they do in Claude Code.
        let local = parse_servers(json["projects"][project].get("mcpServers"), Path::new(project));
        out.extend(local.map_err(|e| format!("{} (project {project}): {e}", path.display()))?);
    }
    // If the config also lists `dogu serve`, leave it out: we do not wrap ourselves.
    out.retain(|_, s| !is_dogu_serve(s));
    Ok(out)
}

fn is_dogu_serve(server: &Server) -> bool {
    match server {
        Server::Stdio { command, args, .. } => {
            Path::new(command).file_name().is_some_and(|f| f == "dogu") && args.iter().any(|a| a == "serve")
        }
        Server::Http { .. } => false,
    }
}

fn parse_servers(servers: Option<&Value>, default_cwd: &Path) -> Result<BTreeMap<String, Server>, String> {
    let servers = match servers {
        None => return Ok(BTreeMap::new()),
        Some(v) => v.as_object().ok_or("\"mcpServers\" must be an object")?,
    };
    let mut out = BTreeMap::new();
    for (name, s) in servers {
        if let Some(server) = parse_server(name, s, default_cwd)? {
            out.insert(name.clone(), server);
        }
    }
    Ok(out)
}

/// One server entry, or None if it is switched off with `"disabled": true`.
fn parse_server(name: &str, s: &Value, default_cwd: &Path) -> Result<Option<Server>, String> {
    if s.get("disabled").and_then(Value::as_bool) == Some(true) {
        return Ok(None);
    }
    if let Some(url) = s.get("url").and_then(Value::as_str) {
        return Ok(Some(Server::Http {
            name: name.to_string(),
            url: expand(url),
            headers: string_map(s.get("headers")),
            oauth: s.get("oauth").cloned().unwrap_or(Value::Null),
        }));
    }
    let Some(command) = s.get("command").and_then(Value::as_str) else {
        return Err(format!("server \"{name}\" needs a \"command\" or a \"url\""));
    };
    let args = s
        .get("args")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(expand).collect())
        .unwrap_or_default();
    let cwd = match s.get("cwd").and_then(Value::as_str) {
        Some(dir) => PathBuf::from(expand(dir)),
        None => default_cwd.to_path_buf(),
    };
    Ok(Some(Server::Stdio { command: expand(command), args, env: string_map(s.get("env")), cwd }))
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
