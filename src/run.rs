//! Code mode: `dogu run program.ts`.
//!
//! The program runs in Deno with every permission switched off: no files, no network,
//! no environment, no other programs. Its only way out is `tools.<app>.<tool>(...)`,
//! which sends the call down a pipe to us. We check it, run it through the same MCP
//! client folder mode uses, and send the answer back.

use crate::call::{log_call, result_text};
use crate::client::{Client, Options as ConnectOptions};
use crate::config::{self, Server};
use crate::sync::{safe, saved_era, schema_path};
use crate::util::base64;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

const RUNNER: &str = include_str!("runner.js");

pub struct Options {
    pub root: PathBuf,
    /// Which tools the program may call: "github", "files/read_*". Empty means all.
    pub allow: Vec<String>,
    pub timeout: Duration,
    pub memory_mb: u32,
    pub max_output: usize,
}

impl Options {
    pub fn new(root: PathBuf) -> Options {
        let env = |name: &str, default: u64| std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default);
        Options {
            root,
            allow: Vec::new(),
            timeout: Duration::from_secs(env("DOGU_TIMEOUT", 120)),
            memory_mb: env("DOGU_MEMORY_MB", 256) as u32,
            max_output: env("DOGU_MAX_OUTPUT", 1_000_000) as usize,
        }
    }
}

/// Open connections to servers, kept so a program's second call to a server is fast.
pub type Pool = HashMap<String, Client>;

pub struct Outcome {
    pub code: i32,
    /// What the program wrote to stderr, if we were asked to collect it.
    pub stderr: String,
}

/// Run `code` in the sandbox. Each line it prints is handed to `out`. Built with the `djs`
/// feature, the sandbox is djs unless `DOGU_SANDBOX=deno`; otherwise it is Deno.
pub fn run_program(
    opts: &Options,
    code: &str,
    pool: &mut Pool,
    collect_stderr: bool,
    out: &mut dyn FnMut(&str),
) -> Result<Outcome, String> {
    #[cfg(feature = "djs")]
    if std::env::var("DOGU_SANDBOX").as_deref() != Ok("deno") {
        return crate::sandbox_djs::run_program(opts, code, pool, collect_stderr, out);
    }
    run_program_deno(opts, code, pool, collect_stderr, out)
}

/// The sandbox on Deno, with every permission off.
fn run_program_deno(
    opts: &Options,
    code: &str,
    pool: &mut Pool,
    collect_stderr: bool,
    out: &mut dyn FnMut(&str),
) -> Result<Outcome, String> {
    let state = crate::sync::saved_state(&opts.root)
        .ok_or_else(|| format!("{} is not a dogu folder. Run `dogu sync` first.", opts.root.display()))?;
    let servers = config::load(&config::Source::from_state(&state)?)?;
    let deno = find_deno()?;

    // Put the program inside the runner and save it to a temp file for Deno.
    let file = TempFile::new(&RUNNER.replace("__PROGRAM__", &base64(code.as_bytes())))?;
    let mut child = Command::new(&deno)
        .args(["run", "--no-prompt", "--no-config", "--no-lock", "--no-npm", "--no-remote", "--deny-import"])
        .arg(format!("--v8-flags=--max-old-space-size={}", opts.memory_mb))
        .arg(&file.0)
        .env_clear() // the program gets none of our environment (it could not read it anyway)
        .env("HOME", config::home())
        .env("NO_COLOR", "1")
        .env("DENO_NO_UPDATE_CHECK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(if collect_stderr { Stdio::piped() } else { Stdio::inherit() })
        .spawn()
        .map_err(|e| format!("cannot start Deno: {e}"))?;
    let mut to_program = child.stdin.take().unwrap();
    let from_program = child.stdout.take().unwrap();

    // Helper threads read the program's output so the main loop can wait with a timeout.
    let (tx, lines) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(from_program);
        let mut buf = Vec::new();
        while matches!(reader.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
            if buf.last() == Some(&b'\n') {
                buf.pop();
            }
            if tx.send(String::from_utf8_lossy(&buf).into_owned()).is_err() {
                break;
            }
            buf.clear();
        }
    });
    let stderr_thread = child.stderr.take().map(|mut e| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = e.read_to_string(&mut text);
            text
        })
    });

    let deadline = Instant::now() + opts.timeout;
    let mut names = Names::default();
    let mut printed = 0;
    let mut timed_out = false;
    loop {
        let line = match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) => line,
            Err(RecvTimeoutError::Disconnected) => break, // the program ended
            Err(RecvTimeoutError::Timeout) => {
                timed_out = true;
                let _ = child.kill();
                break;
            }
        };
        match line.strip_prefix('\x01') {
            // A tool call.
            Some(text) => {
                let Ok(msg) = serde_json::from_str::<Value>(text) else { continue };
                let reply = match tool_call(opts, &servers, pool, &mut names, &msg) {
                    Ok(result) => json!({ "id": msg["id"], "result": result }),
                    Err(e) => json!({ "id": msg["id"], "error": e }),
                };
                let _ = writeln!(to_program, "{reply}"); // fails only if the program is gone
            }
            // Something the program printed.
            None => {
                if printed <= opts.max_output {
                    printed += line.len() + 1;
                    out(&line);
                    if printed > opts.max_output {
                        out(&format!("[output cut after {} bytes. Print less: filter or summarize in the program.]", opts.max_output));
                    }
                }
            }
        }
    }

    drop(to_program);
    let status = child.wait().map_err(|e| e.to_string())?;
    let mut stderr = stderr_thread.and_then(|t| t.join().ok()).unwrap_or_default();
    let mut note = |text: String| if collect_stderr { stderr.push_str(&text) } else { eprintln!("{text}") };
    let code = if timed_out {
        note(format!("error: the program was stopped after {} seconds", opts.timeout.as_secs()));
        124
    } else {
        match status.code() {
            Some(code) => code,
            None => {
                note(format!("error: the program crashed. It may have used more than {} MB of memory.", opts.memory_mb));
                1
            }
        }
    };
    Ok(Outcome { code, stderr })
}

/// Handle one `tools.<app>.<tool>(args)` from the program.
pub(crate) fn tool_call(
    opts: &Options,
    servers: &BTreeMap<String, Server>,
    pool: &mut Pool,
    names: &mut Names,
    msg: &Value,
) -> Result<Value, String> {
    tool_calls(opts, servers, pool, names, std::slice::from_ref(msg)).pop().unwrap_or_else(|| Err("no call".into()))
}

/// What one call from a program needs before it can go to its server.
enum Prepared {
    /// Already answered: a forged tool (run in its own sandbox), or a call we refused.
    Done(Result<Value, String>),
    /// `input_schema` is the tool's `inputSchema` (it says which arguments travel as HTTP headers).
    /// `read_only`: the server marks the tool read-only, so its answers may be kept a short while.
    Call { server: String, tool: String, args: Value, input_schema: Option<Value>, read_only: bool },
}

/// Calls a program made together. Calls to different servers run at the same time, and so do calls
/// to one server where its connection allows (see `Client::call_tools`). Answers come back in order.
pub(crate) fn tool_calls(
    opts: &Options,
    servers: &BTreeMap<String, Server>,
    pool: &mut Pool,
    names: &mut Names,
    msgs: &[Value],
) -> Vec<Result<Value, String>> {
    let prepared: Vec<Prepared> = msgs.iter().map(|m| prepare(opts, servers, pool, names, m)).map(from_cache).collect();
    let prepared = connect_all(opts, servers, pool, prepared);
    let mut by_server: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, p) in prepared.iter().enumerate() {
        if let Prepared::Call { server, .. } = p {
            by_server.entry(server.clone()).or_default().push(i);
        }
    }
    let run_group = |client: &mut Client, idx: &[usize]| -> Vec<Result<Value, String>> {
        let calls = idx
            .iter()
            .map(|&i| match &prepared[i] {
                Prepared::Call { tool, args, input_schema, .. } => (tool.clone(), args.clone(), input_schema.clone()),
                Prepared::Done(_) => unreachable!(),
            })
            .collect();
        client.call_tools(calls)
    };
    let started = Instant::now();
    let answered: Vec<(String, Vec<Result<Value, String>>)> = if by_server.len() == 1 {
        let (server, idx) = by_server.iter().next().unwrap();
        vec![(server.clone(), run_group(pool.get_mut(server).unwrap(), idx))]
    } else {
        std::thread::scope(|scope| {
            let running: Vec<_> = pool
                .iter_mut()
                .filter_map(|(server, client)| by_server.get(server).map(|idx| (server.clone(), client, idx)))
                .map(|(server, client, idx)| (server, scope.spawn(|| run_group(client, idx))))
                .collect();
            running
                .into_iter()
                .map(|(server, h)| {
                    let n = by_server[&server].len();
                    (server, h.join().unwrap_or_else(|_| (0..n).map(|_| Err("a tool call crashed".into())).collect()))
                })
                .collect()
        })
    };
    let mut results: Vec<Option<Result<Value, String>>> = prepared
        .iter()
        .map(|p| match p {
            Prepared::Done(r) => Some(r.clone()),
            Prepared::Call { .. } => None,
        })
        .collect();
    for (server, answers) in answered {
        let mut broken = false;
        for (&i, result) in by_server[&server].iter().zip(answers) {
            let Prepared::Call { tool, args, read_only, .. } = &prepared[i] else { unreachable!() };
            log_call(&opts.root, "run", &server, tool, matches!(&result, Ok(r) if r["isError"] != true), started);
            if let (true, Ok(answer)) = (*read_only, &result) {
                crate::cache::put(&crate::cache::key(&server, tool, args), answer);
            }
            broken |= result.is_err();
            results[i] = Some(result.and_then(useful_form).map_err(|e| with_type(&opts.root, &server, tool, e)));
        }
        if broken {
            pool.remove(&server); // the connection may be broken; start fresh next time
        }
    }
    results.into_iter().map(|r| r.unwrap_or_else(|| Err("no answer".into()))).collect()
}

/// Connections `dogu serve` began as it started and has not used yet. A program that needs one waits
/// for it instead of connecting a second time.
type Warming = Vec<(String, std::thread::JoinHandle<Result<Client, String>>)>;
static WARMING: std::sync::Mutex<Warming> = std::sync::Mutex::new(Vec::new());

/// Start connecting to every server in the background, the way an agent connects to its MCP servers
/// when it starts, so the first program finds them ready. Nothing here opens a login: a server that
/// fails is connected on first use, as before.
pub fn warm_up(opts: &Options) -> Result<(), String> {
    let state = crate::sync::saved_state(&opts.root)
        .ok_or_else(|| format!("{} is not a dogu folder. Run `dogu sync` first.", opts.root.display()))?;
    let servers = config::load(&config::Source::from_state(&state)?)?;
    let mut warming = WARMING.lock().unwrap_or_else(|e| e.into_inner());
    for (name, server) in servers {
        let (root, timeout, key) = (opts.root.clone(), opts.timeout, name.clone());
        let handle = std::thread::spawn(move || {
            let mut options = ConnectOptions::new(timeout);
            options.era = saved_era(&root, &key);
            let mut client = Client::connect(&server, &options)?;
            client.open_now();
            Ok(client)
        });
        warming.push((name, handle));
    }
    Ok(())
}

/// The connection warm_up made for `server`, waiting for it if it is still being made.
fn warmed(server: &str) -> Option<Client> {
    let handle = {
        let mut warming = WARMING.lock().unwrap_or_else(|e| e.into_inner());
        let i = warming.iter().position(|(s, _)| s == server)?;
        warming.remove(i).1
    };
    handle.join().ok()?.ok()
}

/// Connect to every server the calls need that is not connected yet, all at the same time. A call whose
/// server cannot be reached is answered with the reason.
fn connect_all(opts: &Options, servers: &BTreeMap<String, Server>, pool: &mut Pool, prepared: Vec<Prepared>) -> Vec<Prepared> {
    let mut missing: Vec<&str> = prepared
        .iter()
        .filter_map(|p| match p {
            Prepared::Call { server, .. } if !pool.contains_key(server) => Some(server.as_str()),
            _ => None,
        })
        .collect();
    missing.sort();
    missing.dedup();
    let connect = |server: &str| {
        if let Some(client) = warmed(server) {
            return Ok(client);
        }
        let mut options = ConnectOptions::new(opts.timeout);
        options.era = saved_era(&opts.root, server);
        Client::connect(&servers[server], &options)
    };
    let connected: Vec<(String, Result<Client, String>)> = match missing.as_slice() {
        [] => Vec::new(),
        [one] => vec![(one.to_string(), connect(one))],
        many => std::thread::scope(|scope| {
            let running: Vec<_> = many.iter().map(|s| (s.to_string(), scope.spawn(|| connect(s)))).collect();
            running
                .into_iter()
                .map(|(s, h)| (s, h.join().unwrap_or_else(|_| Err("connecting crashed".into()))))
                .collect()
        }),
    };
    let mut failed: BTreeMap<String, String> = BTreeMap::new();
    for (server, result) in connected {
        match result {
            Ok(client) => {
                pool.insert(server, client);
            }
            Err(e) => {
                failed.insert(server, e);
            }
        }
    }
    prepared
        .into_iter()
        .map(|p| match p {
            Prepared::Call { server, .. } if failed.contains_key(&server) => Prepared::Done(Err(failed[&server].clone())),
            other => other,
        })
        .collect()
}

fn prepare(opts: &Options, servers: &BTreeMap<String, Server>, pool: &mut Pool, names: &mut Names, msg: &Value) -> Prepared {
    let args = match &msg["args"] {
        Value::Null => json!({}),
        Value::Object(_) => msg["args"].clone(),
        _ => return Prepared::Done(Err("pass the parameters as one object: tools.app.tool({ name: value })".into())),
    };
    // A forged tool is another program: run it in its own sandbox, sharing our connections.
    if msg["server"].as_str() == Some(crate::sync::FORGED) {
        let tool = msg["tool"].as_str().unwrap_or("");
        if !opts.allow.is_empty() && !opts.allow.iter().any(|p| allows(p, crate::sync::FORGED, tool)) {
            return Prepared::Done(Err(format!(
                "{}/{tool} is not allowed in this run (allowed: {})",
                crate::sync::FORGED,
                opts.allow.join(", ")
            )));
        }
        return Prepared::Done(crate::forge::run(&opts.root, tool, &args, pool).map(|printed| {
            let text = printed.trim_end();
            serde_json::from_str(text).unwrap_or(Value::String(text.to_string()))
        }));
    }
    let (server, tool) = match names.resolve(
        &opts.root,
        servers,
        msg["server"].as_str().unwrap_or(""),
        msg["tool"].as_str().unwrap_or(""),
    ) {
        Ok(found) => found,
        Err(e) => return Prepared::Done(Err(e)),
    };
    // A pattern may use the real names or the names as written in code (`get_sum` for `get-sum`).
    let allowed = |p: &String| allows(p, &server, &tool) || allows(p, &ident(&server), &ident(&tool));
    if !opts.allow.is_empty() && !opts.allow.iter().any(allowed) {
        return Prepared::Done(Err(format!("{server}/{tool} is not allowed in this run (allowed: {})", opts.allow.join(", "))));
    }
    let saved = saved_tool(&opts.root, &server, &tool);
    let read_only = saved.as_ref().is_some_and(crate::cache::is_read_only);
    let input_schema = saved.map(|s| s["inputSchema"].clone()).filter(|s| !s.is_null());
    Prepared::Call { server, tool, args, input_schema, read_only }
}

/// The tool as saved at sync: its schemas and what the server says about it.
pub(crate) fn saved_tool(root: &Path, server: &str, tool: &str) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(schema_path(root, server, tool)).ok()?).ok()
}

/// The `inputSchema` saved for a tool at sync: the client needs it, not the whole saved tool.
#[cfg(test)]
fn input_schema_of(root: &Path, server: &str, tool: &str) -> Option<Value> {
    saved_tool(root, server, tool).map(|s| s["inputSchema"].clone()).filter(|s| !s.is_null())
}

/// A read-only call made with the same arguments a moment ago is answered from memory, without the server.
fn from_cache(p: Prepared) -> Prepared {
    match &p {
        Prepared::Call { server, tool, args, read_only: true, .. } => {
            match crate::cache::get(&crate::cache::key(server, tool, args)) {
                Some(answer) => Prepared::Done(useful_form(answer)),
                None => p,
            }
        }
        _ => p,
    }
}

/// A failed call's error, followed by the tool's full type, so the next try can get the arguments right
/// without a separate look-up.
fn with_type(root: &Path, server: &str, tool: &str, error: String) -> String {
    let saved = std::fs::read_to_string(schema_path(root, server, tool)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    match saved {
        Some(saved) => {
            let decl = crate::sync::tool_decl(&saved, "", &format!("tools.{}.", ident(server)));
            format!("{error}\n\nThe tool's type:\n{}", decl.trim_end())
        }
        None => error,
    }
}

/// Give the program the most useful form of the answer.
fn useful_form(result: Value) -> Result<Value, String> {
    if result["isError"].as_bool() == Some(true) {
        return Err(result_text(&result));
    }
    if !result["structuredContent"].is_null() {
        return Ok(result["structuredContent"].clone());
    }
    let text = result_text(&result);
    Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

/// A name that is safe to write after a dot in code: `get-sum` becomes `get_sum`.
pub fn ident(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '$' { c } else { '_' }).collect();
    if s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{s}")
    } else {
        s
    }
}

/// Finds the real server and tool names for the names a program used.
#[derive(Default)]
pub(crate) struct Names {
    tools: HashMap<String, Vec<String>>,
}

impl Names {
    fn resolve(
        &mut self,
        root: &Path,
        servers: &BTreeMap<String, Server>,
        server: &str,
        tool: &str,
    ) -> Result<(String, String), String> {
        let server = servers
            .keys()
            .find(|k| *k == server)
            .or_else(|| servers.keys().find(|k| ident(k) == server))
            .ok_or_else(|| {
                let known: Vec<String> = servers.keys().map(|k| ident(k)).collect();
                format!("there is no tools.{server}. There is: {}", known.join(", "))
            })?
            .clone();
        let known = self.tools.entry(server.clone()).or_insert_with(|| {
            let dir = root.join(".dogu/schemas").join(safe(&server));
            let mut list = Vec::new();
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let schema: Option<Value> =
                    std::fs::read_to_string(entry.path()).ok().and_then(|t| serde_json::from_str(&t).ok());
                if let Some(name) = schema.as_ref().and_then(|s| s["name"].as_str()) {
                    list.push(name.to_string());
                }
            }
            list
        });
        let found = known.iter().find(|k| *k == tool).or_else(|| known.iter().find(|k| ident(k) == tool));
        match found {
            Some(name) => Ok((server, name.clone())),
            None => Err(format!(
                "there is no tools.{}.{tool}. Read {}/{}/index.d.ts for the list.",
                ident(&server),
                root.display(),
                safe(&server)
            )),
        }
    }
}

/// Does the pattern allow this tool? "github" allows all of github; "*" matches anything.
fn allows(pattern: &str, server: &str, tool: &str) -> bool {
    if pattern.contains('/') {
        glob(pattern.as_bytes(), format!("{server}/{tool}").as_bytes())
    } else {
        glob(pattern.as_bytes(), server.as_bytes())
    }
}

fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len()).any(|i| glob(rest, &text[i..])),
        Some((c, rest)) => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

fn find_deno() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var("DOGU_DENO").ok().filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    dirs.push(config::home().join(".deno/bin"));
    dirs.iter().map(|d| d.join("deno")).find(|p| p.is_file()).ok_or_else(|| {
        "code mode needs Deno, which is the sandbox. Install it (`brew install deno`, or see https://deno.com) \
         and try again."
            .to_string()
    })
}

/// A temp file that is deleted when we are done with it.
struct TempFile(PathBuf);

impl TempFile {
    fn new(text: &str) -> Result<TempFile, String> {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("dogu-run-{}-{stamp}.js", std::process::id()));
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        Ok(TempFile(path))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_client_gets_the_input_schema_not_the_whole_saved_tool() {
        let root = std::env::temp_dir().join(format!("dogu-input-schema-{}", std::process::id()));
        let path = schema_path(&root, "gh", "get_release");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let input = json!({ "type": "object", "properties": { "owner": { "type": "string", "x-mcp-header": "owner" } } });
        std::fs::write(&path, json!({ "name": "get_release", "inputSchema": input }).to_string()).unwrap();

        let got = input_schema_of(&root, "gh", "get_release");

        assert_eq!(got, Some(input));
        assert_eq!(input_schema_of(&root, "gh", "missing"), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
