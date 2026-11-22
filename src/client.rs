//! A small MCP client. It speaks JSON-RPC over stdio or over HTTP.
//! It does four things: connect, list tools, call a tool, hang up.
//!
//! MCP has two eras, and we speak both:
//!   Modern (2026-07-28 and later): no handshake. Every request says which version it speaks.
//!   Legacy (2025-11-25 and earlier): an `initialize` handshake first, then requests.
//! On connect we try modern first and fall back to legacy, as the spec describes.

use crate::auth::{is_local, Auth};
use crate::config::Server;
use crate::util::base64;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};

/// How many of a local server's last stderr lines we keep, to explain it if it dies.
const LAST_WORDS: usize = 6;
use std::time::{Duration, Instant};

pub const MODERN: &str = "2026-07-28";
const LEGACY: &str = "2025-11-25";
const LEGACY_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

// Errors only a modern server sends.
const UNSUPPORTED_VERSION: i64 = -32022;
const HEADER_MISMATCH: i64 = -32020;
const MISSING_CAPABILITY: i64 = -32021;

#[derive(Clone, PartialEq, Debug)]
pub enum Era {
    Modern(String),
    Legacy,
}

impl Era {
    /// How the era is written in our state file.
    pub fn label(&self) -> String {
        match self {
            Era::Modern(v) => v.clone(),
            Era::Legacy => "legacy".into(),
        }
    }
    pub fn from_label(s: &str) -> Era {
        if s == "legacy" {
            Era::Legacy
        } else {
            Era::Modern(s.to_string())
        }
    }
}

pub struct Options {
    pub timeout: Duration,
    /// The era we saw last time, to skip the detection step.
    pub era: Option<Era>,
    /// May we open a browser for a login?
    pub interactive: bool,
}

impl Options {
    pub fn new(timeout: Duration) -> Options {
        Options { timeout, era: None, interactive: false }
    }
}

/// What went wrong with one request.
struct Failure {
    /// The JSON-RPC error code, when the server sent a JSON-RPC error.
    code: Option<i64>,
    message: String,
    data: Value,
    /// True when the server clearly answered "no" (an error reply or an HTTP 4xx),
    /// as opposed to us not reaching it at all.
    answered: bool,
}

impl Failure {
    fn broken(message: impl Into<String>) -> Failure {
        Failure { code: None, message: message.into(), data: Value::Null, answered: false }
    }
}

enum Transport {
    /// `said` keeps the last lines the server wrote to stderr, to explain it if it dies.
    Stdio { child: Child, stdin: ChildStdin, lines: Receiver<String>, said: Arc<Mutex<VecDeque<String>>> },
    Http { agent: ureq::Agent, url: String, headers: BTreeMap<String, String>, session: Option<String>, auth: Option<Auth> },
}

pub struct Client {
    transport: Transport,
    next_id: u64,
    timeout: Duration,
    era: Era,
    /// The version agreed in a legacy handshake (sent as a header on HTTP).
    legacy_version: Option<String>,
    /// What the server said about itself: { serverInfo, instructions }.
    pub info: Value,
}

impl Client {
    pub fn connect(server: &Server, opts: &Options) -> Result<Client, String> {
        let transport = match server {
            Server::Stdio { command, args, env, cwd } => {
                let debug = std::env::var("DOGU_DEBUG").is_ok();
                let mut child = Command::new(command)
                    .args(args)
                    .envs(env)
                    .current_dir(cwd)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(if debug { Stdio::inherit() } else { Stdio::piped() })
                    .spawn()
                    .map_err(|e| format!("cannot start `{command}`: {e}"))?;
                let said = Arc::new(Mutex::new(VecDeque::new()));
                if let Some(stderr) = child.stderr.take() {
                    let said = Arc::clone(&said);
                    std::thread::spawn(move || {
                        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                            let mut said = said.lock().unwrap_or_else(|e| e.into_inner());
                            if said.len() == LAST_WORDS {
                                said.pop_front();
                            }
                            said.push_back(line.chars().take(300).collect());
                        }
                    });
                }
                let stdin = child.stdin.take().unwrap();
                let stdout = child.stdout.take().unwrap();
                // A helper thread reads the server's output, so we can wait with a timeout.
                let (tx, lines) = mpsc::channel();
                std::thread::spawn(move || {
                    for line in BufReader::new(stdout).lines() {
                        let Ok(line) = line else { break };
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                });
                Transport::Stdio { child, stdin, lines, said }
            }
            Server::Http { name, url, headers, oauth } => {
                // If the config already gives an Authorization header, we leave logins alone.
                let own_auth = headers.keys().any(|k| k.eq_ignore_ascii_case("authorization"));
                Transport::Http {
                    agent: ureq::AgentBuilder::new()
                        .try_proxy_from_env(!is_local(url)) // never send localhost through a proxy
                        .timeout_connect(Duration::from_secs(15))
                        .timeout_read(opts.timeout)
                        .build(),
                    url: url.clone(),
                    headers: headers.clone(),
                    session: None,
                    auth: (!own_auth).then(|| Auth::new(name, url, oauth, opts.interactive)),
                }
            }
        };
        let mut client = Client {
            transport,
            next_id: 1,
            timeout: opts.timeout,
            era: Era::Modern(MODERN.into()),
            legacy_version: None,
            info: Value::Null,
        };
        let connected = match &opts.era {
            // We know this server is modern: there is no handshake, so we are connected.
            Some(Era::Modern(version)) => {
                client.era = Era::Modern(version.clone());
                Ok(())
            }
            // We know it is legacy: go straight to the handshake. If that fails, detect again.
            Some(Era::Legacy) => client.initialize().or_else(|_| client.detect()),
            None => client.detect(),
        };
        match connected {
            Ok(()) => Ok(client),
            Err(e) => Err(client.explain(e)),
        }
    }

    /// If a local server has died, add what it said last, and a hint for a common cause.
    fn explain(&mut self, error: String) -> String {
        let Transport::Stdio { child, said, .. } = &mut self.transport else { return error };
        // Give the stderr reader a moment to collect the server's last lines.
        for _ in 0..20 {
            if matches!(child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        if !matches!(child.try_wait(), Ok(Some(_))) {
            return error;
        }
        std::thread::sleep(Duration::from_millis(50));
        let words: Vec<String> = said.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect();
        if words.is_empty() {
            return error;
        }
        let text = words.join("\n  ");
        let hint = if text.contains("EPERM") && text.contains(".npm") {
            "\n  It could not write npm's cache (~/.npm). That happens inside a sandbox, such as Codex's: allow writing to ~/.npm."
        } else {
            ""
        };
        format!("{}. The server said:\n  {text}{hint}", error.trim_end_matches(" (set DOGU_DEBUG=1 to see its output)"))
    }

    pub fn era(&self) -> &Era {
        &self.era
    }

    /// Find out which era the server speaks.
    fn detect(&mut self) -> Result<(), String> {
        self.detect_once(false)
    }

    fn detect_once(&mut self, retried: bool) -> Result<(), String> {
        self.era = Era::Modern(MODERN.into());
        let probe = match &self.transport {
            Transport::Http { .. } => self.send("server/discover", json!({}), &[]),
            Transport::Stdio { .. } => self.stdio_probe(),
        };
        match probe {
            // A modern server. It tells us which versions it speaks.
            Ok(result) => {
                let versions: Vec<&str> = result["supportedVersions"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                if result.get("supportedVersions").is_none() && result.get("protocolVersion").is_some() {
                    // This is the answer to `initialize` (see stdio_probe): a legacy server.
                    return self.finish_initialize(result);
                }
                if !versions.is_empty() && !versions.contains(&MODERN) {
                    return self.initialize().map_err(|_| {
                        format!("the server speaks MCP versions {versions:?}, and dogu does not know any of them")
                    });
                }
                self.info = json!({
                    "serverInfo": result["_meta"]["io.modelcontextprotocol/serverInfo"],
                    "instructions": result["instructions"],
                });
                Ok(())
            }
            // A modern server that refused the version we asked for. It lists what it speaks.
            Err(f) if f.code == Some(UNSUPPORTED_VERSION) => {
                if std::env::var("DOGU_DEBUG").is_ok() {
                    eprintln!("dogu: version refused, server data: {}", f.data);
                }
                let offered: Vec<&str> = f.data["supported"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                let unknown = format!("the server speaks MCP versions {offered:?}, and dogu does not know any of them");
                if offered.contains(&MODERN) && !retried {
                    return self.detect_once(true); // it does speak ours: ask again
                }
                if offered.iter().any(|v| LEGACY_VERSIONS.contains(v)) {
                    return self.initialize().map_err(|_| unknown);
                }
                Err(unknown)
            }
            // A modern server, but our request was wrong. Falling back would not help.
            Err(f) if matches!(f.code, Some(HEADER_MISMATCH) | Some(MISSING_CAPABILITY)) => Err(f.message),
            // The server said no in some other way: it is legacy. Do the handshake.
            Err(f) if f.answered => self.initialize(),
            // We could not talk to it at all.
            Err(f) => Err(f.message),
        }
    }

    /// On stdio there is no HTTP status to tell the eras apart, so we ask `server/discover` first.
    /// A legacy server may not answer that at all. So after a short wait we also send `initialize`
    /// and take whichever answer comes.
    fn stdio_probe(&mut self) -> Result<Value, Failure> {
        let discover_id = self.next_id;
        let init_id = self.next_id + 1;
        self.next_id += 2;
        let discover = json!({ "jsonrpc": "2.0", "id": discover_id, "method": "server/discover",
                               "params": { "_meta": meta(MODERN) } });
        let deadline = Instant::now() + self.timeout;
        let Transport::Stdio { stdin, lines, .. } = &mut self.transport else { unreachable!() };
        send_line(stdin, &discover).map_err(Failure::broken)?;

        let quiet_limit = Instant::now() + Duration::from_secs(5).min(self.timeout);
        let mut init_sent = false;
        let mut discover_failed = false;
        let mut init_failure: Option<Failure> = None;
        loop {
            let limit = if init_sent { deadline } else { quiet_limit };
            match wait_for(stdin, lines, &[discover_id, init_id], limit) {
                Some((id, Ok(result))) if id == discover_id => return Ok(result),
                Some((_, Ok(result))) => return Ok(result), // the initialize answer
                Some((id, Err(f))) if id == discover_id => {
                    if f.code == Some(UNSUPPORTED_VERSION) || !f.answered {
                        return Err(f);
                    }
                    discover_failed = true; // legacy
                }
                Some((_, Err(f))) => init_failure = Some(f),
                None if init_sent => return Err(Failure::broken("the server did not answer in time")),
                None => {}
            }
            if !init_sent {
                let init = json!({ "jsonrpc": "2.0", "id": init_id, "method": "initialize", "params": init_params() });
                send_line(stdin, &init).map_err(Failure::broken)?;
                init_sent = true;
            }
            if discover_failed && init_failure.is_some() {
                return Err(Failure::broken(format!("the server refused to connect: {}", init_failure.unwrap().message)));
            }
        }
    }

    /// The legacy handshake.
    fn initialize(&mut self) -> Result<(), String> {
        self.era = Era::Legacy;
        self.legacy_version = None;
        let result = self.send("initialize", init_params(), &[]).map_err(|f| f.message)?;
        self.finish_initialize(result)
    }

    fn finish_initialize(&mut self, result: Value) -> Result<(), String> {
        self.era = Era::Legacy;
        self.legacy_version = result["protocolVersion"].as_str().map(String::from);
        self.info = result;
        self.notify("notifications/initialized")
    }

    /// All tools the server has. Servers may answer in pages, so keep asking until done.
    pub fn list_tools(&mut self) -> Result<Vec<Value>, String> {
        let result = self.list_tools_inner();
        result.map_err(|e| self.explain(e))
    }

    fn list_tools_inner(&mut self) -> Result<Vec<Value>, String> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let params = match &cursor {
                Some(c) => json!({ "cursor": c }),
                None => json!({}),
            };
            let page = self.request("tools/list", params, &[])?;
            if let Some(list) = page["tools"].as_array() {
                tools.extend(list.iter().cloned());
            }
            cursor = page["nextCursor"].as_str().map(String::from);
            if cursor.is_none() {
                return Ok(tools);
            }
        }
    }

    /// Run a tool. `input_schema` is the tool's schema, if we have it: a modern HTTP server may
    /// mark parameters there that must also be sent as HTTP headers.
    pub fn call_tool(&mut self, name: &str, arguments: Value, input_schema: Option<&Value>) -> Result<Value, String> {
        let result = self.call_tool_inner(name, arguments, input_schema);
        result.map_err(|e| self.explain(e))
    }

    /// Several calls at once. On HTTP each goes on its own request; over stdio they are all written
    /// first and the answers collected as they come, so a server that can work on them together does.
    /// A server with a login (which may need refreshing) gets its calls one after another.
    /// Each call is (tool name, arguments, the tool's `inputSchema`).
    pub fn call_tools(&mut self, calls: Vec<(String, Value, Option<Value>)>) -> Vec<Result<Value, String>> {
        if calls.len() < 2 {
            return calls.into_iter().map(|(name, args, schema)| self.call_tool(&name, args, schema.as_ref())).collect();
        }
        match &self.transport {
            Transport::Http { auth: None, .. } => {
                let forks: Vec<Client> = (0..calls.len()).filter_map(|i| self.fork(i)).collect();
                std::thread::scope(|scope| {
                    let running: Vec<_> = forks
                        .into_iter()
                        .zip(calls)
                        .map(|(mut fork, (name, args, schema))| {
                            scope.spawn(move || fork.call_tool(&name, args, schema.as_ref()))
                        })
                        .collect();
                    running.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a tool call crashed".into()))).collect()
                })
            }
            Transport::Stdio { .. } => self.call_tools_pipelined(calls),
            Transport::Http { .. } => {
                calls.into_iter().map(|(name, args, schema)| self.call_tool(&name, args, schema.as_ref())).collect()
            }
        }
    }

    /// A copy of an HTTP connection without a login, for one call on another thread.
    fn fork(&self, n: usize) -> Option<Client> {
        let Transport::Http { agent, url, headers, session, auth: None } = &self.transport else { return None };
        Some(Client {
            transport: Transport::Http {
                agent: agent.clone(),
                url: url.clone(),
                headers: headers.clone(),
                session: session.clone(),
                auth: None,
            },
            next_id: self.next_id + 1_000 * (n as u64 + 1),
            timeout: self.timeout,
            era: self.era.clone(),
            legacy_version: self.legacy_version.clone(),
            info: Value::Null,
        })
    }

    fn call_tools_pipelined(&mut self, calls: Vec<(String, Value, Option<Value>)>) -> Vec<Result<Value, String>> {
        let deadline = Instant::now() + self.timeout;
        let mut ids = Vec::new();
        for (name, args, _) in &calls {
            let id = self.next_id;
            self.next_id += 1;
            let mut params = json!({ "name": name, "arguments": args });
            if let Era::Modern(version) = &self.era {
                params["_meta"] = meta(version);
            }
            let msg = json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params });
            let Transport::Stdio { stdin, .. } = &mut self.transport else { unreachable!() };
            if let Err(e) = send_line(stdin, &msg) {
                let e = self.explain(e);
                return calls.iter().map(|_| Err(e.clone())).collect();
            }
            ids.push(id);
        }
        let mut answers: HashMap<u64, Result<Value, Failure>> = HashMap::new();
        while answers.len() < ids.len() {
            let waiting: Vec<u64> = ids.iter().copied().filter(|id| !answers.contains_key(id)).collect();
            let Transport::Stdio { stdin, lines, .. } = &mut self.transport else { unreachable!() };
            match wait_for(stdin, lines, &waiting, deadline) {
                Some((id, answer)) => {
                    let broken = matches!(&answer, Err(f) if !f.answered);
                    answers.insert(id, answer);
                    if broken {
                        break; // the server exited: the rest will not come
                    }
                }
                None => break,
            }
        }
        // An answer that needs another round (a newer version, "ask me again") goes the usual way.
        ids.iter()
            .zip(calls)
            .map(|(id, (name, args, schema))| match answers.remove(id) {
                Some(Ok(r)) if r["resultType"] != "input_required" => Ok(r),
                Some(Err(f)) if f.code != Some(UNSUPPORTED_VERSION) => Err(self.explain(f.message)),
                Some(_) => self.call_tool(&name, args, schema.as_ref()),
                None => Err(self.explain("no answer to tools/call in time".to_string())),
            })
            .collect()
    }

    fn call_tool_inner(&mut self, name: &str, arguments: Value, input_schema: Option<&Value>) -> Result<Value, String> {
        let mut headers = Vec::new();
        if let Some(schema) = input_schema {
            for (path, header) in header_params(schema).unwrap_or_default() {
                let mut value = &arguments;
                for key in &path {
                    value = &value[key];
                }
                let text = match value {
                    Value::String(s) => s.clone(),
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => match n.as_f64() {
                        Some(f) if n.as_i64().is_none() && n.as_u64().is_none() && f.fract() == 0.0 => {
                            format!("{}", f as i64)
                        }
                        _ => n.to_string(),
                    },
                    _ => continue, // not given, or null: no header
                };
                headers.push((format!("Mcp-Param-{header}"), header_value(&text)));
            }
        }
        self.request("tools/call", json!({ "name": name, "arguments": arguments }), &headers)
    }

    fn request(&mut self, method: &str, params: Value, headers: &[(String, String)]) -> Result<Value, String> {
        let result = match self.send(method, params.clone(), headers) {
            Ok(r) => r,
            // A modern server that stopped speaking the version we remembered: detect again, once.
            Err(f) if f.code == Some(UNSUPPORTED_VERSION) => {
                self.detect()?;
                self.send(method, params.clone(), headers).map_err(|f| f.message)?
            }
            Err(f) => return Err(f.message),
        };
        // A modern server may answer "not done yet, ask me again". If it only hands us a state
        // string to send back, we do that. If it wants input from the user (a form to fill in, an
        // LLM completion), we cannot give it: we never said we could, so a server should not ask.
        let mut result = result;
        for _ in 0..5 {
            if result["resultType"] != "input_required" {
                return Ok(result);
            }
            if result["inputRequests"].as_object().map_or(false, |r| !r.is_empty()) {
                if std::env::var("DOGU_DEBUG").is_ok() {
                    eprintln!("dogu: input required: {result}");
                }
                return Err("the server asked the user a question to finish this, and dogu cannot answer those yet".into());
            }
            let mut again = params.clone();
            if let Some(state) = result.get("requestState") {
                again["requestState"] = state.clone(); // sent back exactly as it came
            }
            result = self.send(method, again, headers).map_err(|f| f.message)?;
        }
        Err("the server kept asking to be called again. Giving up.".into())
    }

    /// Send one request in the current era and wait for its answer.
    fn send(&mut self, method: &str, mut params: Value, extra_headers: &[(String, String)]) -> Result<Value, Failure> {
        let id = self.next_id;
        self.next_id += 1;
        let mut headers: Vec<(String, String)> = Vec::new();
        match &self.era {
            Era::Modern(version) => {
                // No handshake happened, so every request carries the version and who we are.
                params["_meta"] = meta(version);
                headers.push(("MCP-Protocol-Version".into(), version.clone()));
                headers.push(("Mcp-Method".into(), method.into()));
                if let Some(name) = params["name"].as_str().or(params["uri"].as_str()) {
                    headers.push(("Mcp-Name".into(), header_value(name)));
                }
                headers.extend_from_slice(extra_headers);
            }
            Era::Legacy => {
                if let Some(version) = &self.legacy_version {
                    headers.push(("MCP-Protocol-Version".into(), version.clone()));
                }
            }
        }
        let legacy = self.era == Era::Legacy;
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let deadline = Instant::now() + self.timeout;

        match &mut self.transport {
            Transport::Stdio { stdin, lines, .. } => {
                send_line(stdin, &msg).map_err(Failure::broken)?;
                match wait_for(stdin, lines, &[id], deadline) {
                    Some((_, answer)) => answer,
                    None => Err(Failure::broken(format!("no answer to {method} in time"))),
                }
            }
            Transport::Http { agent, url, headers: fixed, session, auth } => loop {
                let mut req = agent
                    .post(url)
                    .set("Content-Type", "application/json")
                    .set("Accept", "application/json, text/event-stream");
                for (k, v) in fixed.iter() {
                    req = req.set(k, v);
                }
                for (k, v) in &headers {
                    req = req.set(k, v);
                }
                if let (true, Some(s)) = (legacy, session.as_ref()) {
                    req = req.set("Mcp-Session-Id", s);
                }
                if let Some(token) = auth.as_mut().and_then(|a| a.bearer()) {
                    req = req.set("Authorization", &format!("Bearer {token}"));
                }
                let resp = match req.send_string(&msg.to_string()) {
                    Ok(resp) => resp,
                    Err(ureq::Error::Status(code, resp)) => {
                        let challenge = resp.header("www-authenticate").map(String::from);
                        match (code, auth.as_mut()) {
                            // Needs a login (or a fresh token). Sort that out and try again.
                            (401, Some(auth)) => {
                                auth.unauthorized(challenge.as_deref()).map_err(Failure::broken)?;
                                continue;
                            }
                            // Needs more permissions than our token has.
                            (403, Some(auth)) => {
                                if auth.forbidden(challenge.as_deref()).map_err(Failure::broken)? {
                                    continue;
                                }
                            }
                            (401, None) | (403, None) => {
                                return Err(Failure::broken("the server refused the Authorization header from your config"))
                            }
                            _ => {}
                        }
                        // Any other "no". A modern server puts a JSON-RPC error in the body.
                        let body = resp.into_string().unwrap_or_default();
                        let error = serde_json::from_str::<Value>(&body).ok().map(|v| v["error"].clone());
                        return Err(match error {
                            Some(e) if e.is_object() => Failure {
                                code: e["code"].as_i64(),
                                message: e["message"].as_str().unwrap_or("server error").to_string(),
                                data: e["data"].clone(),
                                answered: true,
                            },
                            _ => Failure {
                                code: None,
                                message: format!("HTTP {code} {}", body.chars().take(200).collect::<String>()),
                                data: Value::Null,
                                answered: (400..500).contains(&code),
                            },
                        });
                    }
                    Err(e) => return Err(Failure::broken(e.to_string())),
                };
                if let Some(auth) = auth.as_mut() {
                    auth.ok();
                }
                if let (true, Some(s)) = (legacy, resp.header("mcp-session-id")) {
                    *session = Some(s.to_string());
                }
                if resp.content_type() == "text/event-stream" {
                    // The answer arrives as a stream of events. Read until ours shows up.
                    let mut data = String::new();
                    for line in BufReader::new(resp.into_reader()).lines() {
                        let line = line.map_err(|e| Failure::broken(format!("connection lost: {e}")))?;
                        if let Some(d) = line.strip_prefix("data:") {
                            if !data.is_empty() {
                                data.push('\n');
                            }
                            data.push_str(d.trim_start());
                        } else if line.is_empty() && !data.is_empty() {
                            if let Ok(v) = serde_json::from_str::<Value>(&data) {
                                if let Some(answer) = answer_for(&v, id) {
                                    return answer;
                                }
                            }
                            data.clear();
                        }
                    }
                    return Err(Failure::broken(format!("the server closed the stream without answering {method}")));
                }
                let body = resp.into_string().map_err(|e| Failure::broken(e.to_string()))?;
                let v: Value = serde_json::from_str(&body)
                    .map_err(|_| Failure::broken("the server did not answer with JSON"))?;
                return answer_for(&v, id).unwrap_or_else(|| Err(Failure::broken("unexpected answer from server")));
            },
        }
    }

    /// Send a message that needs no answer (legacy era only).
    fn notify(&mut self, method: &str) -> Result<(), String> {
        let msg = json!({ "jsonrpc": "2.0", "method": method });
        let version = self.legacy_version.clone();
        match &mut self.transport {
            Transport::Stdio { stdin, .. } => send_line(stdin, &msg),
            Transport::Http { agent, url, headers, session, auth } => {
                let mut req = agent
                    .post(url)
                    .set("Content-Type", "application/json")
                    .set("Accept", "application/json, text/event-stream");
                for (k, v) in headers.iter() {
                    req = req.set(k, v);
                }
                if let Some(v) = &version {
                    req = req.set("MCP-Protocol-Version", v);
                }
                if let Some(s) = session {
                    req = req.set("Mcp-Session-Id", s);
                }
                if let Some(token) = auth.as_mut().and_then(|a| a.bearer()) {
                    req = req.set("Authorization", &format!("Bearer {token}"));
                }
                req.send_string(&msg.to_string()).map(|_| ()).map_err(|e| e.to_string())
            }
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Transport::Stdio { child, .. } = &mut self.transport {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The `_meta` every modern request carries: the version, who we are, what we can do.
fn meta(version: &str) -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": version,
        "io.modelcontextprotocol/clientInfo": { "name": "dogu", "version": env!("CARGO_PKG_VERSION") },
        "io.modelcontextprotocol/clientCapabilities": {},
    })
}

fn init_params() -> Value {
    json!({
        "protocolVersion": LEGACY,
        "capabilities": {},
        "clientInfo": { "name": "dogu", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn send_line(stdin: &mut ChildStdin, msg: &Value) -> Result<(), String> {
    writeln!(stdin, "{msg}").and_then(|_| stdin.flush()).map_err(|e| format!("cannot write to server: {e}"))
}

/// Read the server's output until the answer to one of `ids` arrives, or time runs out.
fn wait_for(
    stdin: &mut ChildStdin,
    lines: &Receiver<String>,
    ids: &[u64],
    deadline: Instant,
) -> Option<(u64, Result<Value, Failure>)> {
    loop {
        let wait = deadline.saturating_duration_since(Instant::now());
        let line = match lines.recv_timeout(wait) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => return None,
            Err(RecvTimeoutError::Disconnected) => {
                let f = Failure::broken("the server exited (set DOGU_DEBUG=1 to see its output)");
                return Some((ids[0], Err(f)));
            }
        };
        // Skip anything that is not JSON (some servers print banners).
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        for id in ids {
            if let Some(answer) = answer_for(&v, *id) {
                return Some((*id, answer));
            }
        }
        // A legacy server may ask *us* something. We only know "ping".
        if v.get("method").is_some() && v.get("id").is_some() {
            let reply = if v["method"] == "ping" {
                json!({ "jsonrpc": "2.0", "id": v["id"], "result": {} })
            } else {
                json!({ "jsonrpc": "2.0", "id": v["id"],
                        "error": { "code": -32601, "message": "not supported by dogu" } })
            };
            let _ = send_line(stdin, &reply);
        }
    }
}

/// If `v` is the answer to request `id`, return its result (or its error).
fn answer_for(v: &Value, id: u64) -> Option<Result<Value, Failure>> {
    if v.get("method").is_some() || v["id"].as_u64() != Some(id) {
        return None;
    }
    Some(match v.get("error") {
        Some(e) => Err(Failure {
            code: e["code"].as_i64(),
            message: e["message"].as_str().unwrap_or("server error").to_string(),
            data: e["data"].clone(),
            answered: true,
        }),
        None => Ok(v["result"].clone()),
    })
}

/// Make a value safe for an HTTP header. Plain visible ASCII goes as it is.
/// Anything else is sent as `=?base64?...?=`, as the spec says.
fn header_value(text: &str) -> String {
    let plain = text.bytes().all(|b| (0x20..=0x7e).contains(&b) || b == b'\t')
        && text.trim() == text
        && !(text.starts_with("=?base64?") && text.ends_with("?="));
    if plain {
        text.to_string()
    } else {
        format!("=?base64?{}?=", base64(text.as_bytes()))
    }
}

/// Find the parameters a tool marks with `x-mcp-header`: (path to the value, header name).
/// Returns an error if the tool uses the mark in a way the spec forbids. Such a tool must not be used.
pub fn header_params(schema: &Value) -> Result<Vec<(Vec<String>, String)>, String> {
    fn walk(schema: &Value, reachable: bool, path: &mut Vec<String>, out: &mut Vec<(Vec<String>, String)>) -> Result<(), String> {
        let Some(obj) = schema.as_object() else { return Ok(()) };
        if let Some(mark) = obj.get("x-mcp-header") {
            let name = mark.as_str().unwrap_or("");
            let token = !name.is_empty()
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b));
            if !token {
                return Err("an x-mcp-header name is empty or has characters a header name cannot have".into());
            }
            if !reachable || path.is_empty() {
                return Err(format!("x-mcp-header \"{name}\" is in a place the spec does not allow"));
            }
            if !matches!(obj.get("type").and_then(Value::as_str), Some("string" | "integer" | "boolean")) {
                return Err(format!("x-mcp-header \"{name}\" is on a parameter that is not a string, integer or boolean"));
            }
            if out.iter().any(|(_, have)| have.eq_ignore_ascii_case(name)) {
                return Err(format!("x-mcp-header \"{name}\" is used twice"));
            }
            out.push((path.clone(), name.to_string()));
        }
        for (key, value) in obj {
            match key.as_str() {
                "properties" => {
                    for (prop, sub) in value.as_object().into_iter().flatten() {
                        path.push(prop.clone());
                        walk(sub, reachable, path, out)?;
                        path.pop();
                    }
                }
                // Marks below any of these are not allowed. Walk them only to catch that.
                "items" | "prefixItems" | "anyOf" | "oneOf" | "allOf" | "not" | "if" | "then" | "else"
                | "additionalProperties" | "patternProperties" | "$defs" | "definitions" | "contains"
                | "dependentSchemas" | "propertyNames" | "unevaluatedItems" | "unevaluatedProperties" => {
                    let subs: Vec<&Value> = match value {
                        Value::Array(list) => list.iter().collect(),
                        Value::Object(map) if matches!(key.as_str(), "patternProperties" | "$defs" | "definitions" | "dependentSchemas") => {
                            map.values().collect()
                        }
                        other => vec![other],
                    };
                    for sub in subs {
                        walk(sub, false, path, out)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(schema, true, &mut Vec::new(), &mut out)?;
    Ok(out)
}
