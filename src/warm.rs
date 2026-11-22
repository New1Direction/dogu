//! A warm folder: one background dogu per folder keeps the connections to its servers open, so that
//! `dogu call`, a new process every time, does not connect again on every call (for a local server
//! started with npx that is about a second each time).
//!
//! `dogu call` hands the call to it over a socket that only you can use, in `/tmp/dogu-<your uid>/` (a
//! socket's path must be short, so it cannot live in the folder); `.dogu/warm.json` in the folder says
//! where it is while the process runs. If nothing is listening, the call is made directly as before, and the background process is
//! started for the next call. It steps aside whenever it could be wrong: when the folder was synced
//! again, or when the servers' settings (an environment variable such as a token) differ from the ones
//! it started with. It never sends secrets over the socket, only a fingerprint to compare. It exits
//! after DOGU_WARM_IDLE seconds without a call (600 by default). DOGU_WARM=0 turns all of this off.

use crate::client::{Client, Options};
use crate::config::{self, Server};
use crate::sync::{saved_era, saved_state};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// How long the background process waits for a call before it exits.
const IDLE: Duration = Duration::from_secs(600);
/// How long a call waits to reach the background process before it calls directly.
const REACH: Duration = Duration::from_millis(200);

/// The socket for a folder, named by its path, in a directory only you can use. `None` when that
/// directory cannot be made safe (someone else owns it, or others may read it): then no warm process.
fn socket(root: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let me = std::fs::metadata(root.join(".dogu")).ok()?.uid();
    let dir = PathBuf::from(format!("/tmp/dogu-{me}"));
    let _ = std::fs::DirBuilder::new().mode(0o700).create(&dir);
    let meta = std::fs::symlink_metadata(&dir).ok()?;
    if !meta.is_dir() || meta.uid() != me || meta.permissions().mode() & 0o077 != 0 {
        return None;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()).hash(&mut h);
    Some(dir.join(format!("{:016x}.sock", h.finish())))
}

/// Says, in the folder, that a background process is running and where its socket is.
fn marker(root: &Path) -> PathBuf {
    root.join(".dogu/warm.json")
}

pub fn enabled() -> bool {
    std::env::var("DOGU_WARM").as_deref() != Ok("0")
}

/// What the servers' settings come to after `${VAR}`s are filled in, as one number. Two processes with
/// the same number would connect the same way. The settings themselves stay in each process.
fn fingerprint(servers: &BTreeMap<String, Server>) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (name, server) in servers {
        name.hash(&mut h);
        match server {
            Server::Stdio { command, args, env, cwd } => ("stdio", command, args, env, cwd).hash(&mut h),
            Server::Http { name, url, headers, oauth } => ("http", name, url, headers, oauth.to_string()).hash(&mut h),
        }
    }
    format!("{:016x}", h.finish())
}

/// When the folder was last synced: a new sync means new servers or tools.
fn synced_at(root: &Path) -> Option<SystemTime> {
    std::fs::metadata(root.join(".dogu/state.json")).and_then(|m| m.modified()).ok()
}

fn servers_of(root: &Path) -> Result<BTreeMap<String, Server>, String> {
    config::load(&config::Source::from_state(&saved_state(root).unwrap_or(Value::Null))?)
}

/// Make the call through the folder's background process. `None` means it could not, and the caller
/// should call directly (it may have started the background process for next time).
pub fn call(root: &Path, server: &str, tool: &str, arguments: &Value) -> Option<Result<Value, String>> {
    if !enabled() {
        return None;
    }
    let print = servers_of(root).ok().map(|s| fingerprint(&s))?;
    let Ok(stream) = UnixStream::connect(socket(root)?) else {
        start(root);
        return None;
    };
    let request = json!({ "server": server, "tool": tool, "arguments": arguments, "fingerprint": print });
    let answer = ask(stream, &request)?;
    match (answer.get("result"), answer["error"].as_str(), answer["stale"].as_bool()) {
        (_, _, Some(true)) => None, // it stepped aside, and exits; the next call starts a fresh one
        (Some(result), _, _) => Some(Ok(result.clone())),
        (_, Some(error), _) => Some(Err(error.to_string())),
        _ => None,
    }
}

fn ask(mut stream: UnixStream, request: &Value) -> Option<Value> {
    let timeout = std::env::var("DOGU_TIMEOUT").ok().and_then(|s| s.parse().ok()).unwrap_or(300);
    stream.set_write_timeout(Some(REACH)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(timeout))).ok()?;
    writeln!(stream, "{request}").ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

/// Start the background process for this folder, detached: in its own process group, so the shell
/// that ran this call does not take it down when it ends.
fn start(root: &Path) {
    use std::os::unix::process::CommandExt;
    let Ok(me) = std::env::current_exe() else { return };
    let _ = std::process::Command::new(me)
        .args(["warm", "--root"])
        .arg(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn();
}

/// Ready connections per server. A call takes one out (or makes one), so calls to one server can run
/// at the same time, and puts it back when it worked.
type Pool = Mutex<HashMap<String, Vec<Client>>>;

/// `dogu warm --root <folder>`: the background process. Started by `dogu call`; not meant to be run by hand.
pub fn serve(root: &Path) -> Result<(), String> {
    let path = socket(root).ok_or("no private place for the socket: /tmp/dogu-<uid> is not yours alone")?;
    if UnixStream::connect(&path).is_ok() {
        return Ok(()); // one is already running
    }
    let _ = std::fs::remove_file(&path); // left by one that ended without cleaning up
    let listener = UnixListener::bind(&path).map_err(|e| format!("cannot listen on {}: {e}", path.display()))?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    let note = json!({ "pid": std::process::id(), "socket": path.display().to_string() });
    std::fs::write(marker(root), note.to_string()).map_err(|e| e.to_string())?;
    let servers = servers_of(root)?;
    let print = fingerprint(&servers);
    let synced = synced_at(root);
    let idle = std::env::var("DOGU_WARM_IDLE").ok().and_then(|s| s.parse().ok()).map(Duration::from_secs).unwrap_or(IDLE);
    let last = Arc::new(Mutex::new(Instant::now()));
    let pool: Arc<Pool> = Arc::new(Mutex::new(HashMap::new()));
    let servers = Arc::new(servers);

    // Connect to every server now, while the agent reads the first answer, so the next call finds them ready.
    for name in servers.keys() {
        let (name, servers, pool, root) = (name.clone(), Arc::clone(&servers), Arc::clone(&pool), root.to_path_buf());
        std::thread::spawn(move || {
            let mut options = Options::new(Duration::from_secs(60));
            options.era = saved_era(&root, &name);
            if let Ok(mut client) = Client::connect(&servers[&name], &options) {
                client.open_now();
                if let Ok(mut p) = pool.lock() {
                    p.entry(name).or_default().push(client);
                }
            }
        });
    }

    // Leave when idle, or when the folder is gone or synced again.
    {
        let (last, path, root, pool) = (Arc::clone(&last), path.clone(), root.to_path_buf(), Arc::clone(&pool));
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500).min(idle));
            let quiet = last.lock().map(|t| t.elapsed() >= idle).unwrap_or(true);
            if quiet || synced_at(&root) != synced || !path.exists() || !marker(&root).exists() {
                leave(&root, &path, &pool);
            }
        });
    }

    for stream in listener.incoming().flatten() {
        if let Ok(mut t) = last.lock() {
            *t = Instant::now();
        }
        let (root, servers, pool, print, last, path) =
            (root.to_path_buf(), Arc::clone(&servers), Arc::clone(&pool), print.clone(), Arc::clone(&last), path.clone());
        std::thread::spawn(move || {
            let answer = handle(&root, &servers, &pool, &print, &stream);
            let mut out = &stream;
            let _ = writeln!(out, "{answer}");
            if let Ok(mut t) = last.lock() {
                *t = Instant::now();
            }
            if answer["stale"] == true {
                leave(&root, &path, &pool);
            }
        });
    }
    Ok(())
}

/// Stop: remove the socket and the note in the folder, and close every connection, which ends the local
/// servers this process started, then exit. (Exiting alone would leave those servers running.)
fn leave(root: &Path, path: &Path, pool: &Pool) -> ! {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(marker(root));
    if let Ok(mut p) = pool.lock() {
        p.clear();
    }
    std::process::exit(0)
}

fn handle(root: &Path, servers: &BTreeMap<String, Server>, pool: &Pool, print: &str, stream: &UnixStream) -> Value {
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return json!({ "error": "could not read the call" });
    }
    let Ok(request) = serde_json::from_str::<Value>(&line) else { return json!({ "error": "not JSON" }) };
    // Different settings, or the folder was synced again: the caller should call directly.
    if request["fingerprint"].as_str() != Some(print) {
        return json!({ "stale": true });
    }
    let (server, tool) = (request["server"].as_str().unwrap_or(""), request["tool"].as_str().unwrap_or(""));
    let Some(spec) = servers.get(server) else { return json!({ "stale": true }) };
    let input: Value = std::fs::read_to_string(crate::sync::schema_path(root, server, tool))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .map(|s| s["inputSchema"].clone())
        .unwrap_or(Value::Null);
    let ready = pool.lock().ok().and_then(|mut p| p.get_mut(server).and_then(Vec::pop));
    let client = match ready {
        Some(c) => Ok(c),
        None => {
            let secs = std::env::var("DOGU_TIMEOUT").ok().and_then(|s| s.parse().ok()).unwrap_or(300);
            let mut options = Options::new(Duration::from_secs(secs));
            options.era = saved_era(root, server);
            Client::connect(spec, &options)
        }
    };
    let mut client = match client {
        Ok(c) => c,
        Err(e) => return json!({ "error": e }),
    };
    match client.call_tool(tool, request["arguments"].clone(), Some(&input)) {
        Ok(result) => {
            if let Ok(mut p) = pool.lock() {
                p.entry(server.to_string()).or_default().push(client);
            }
            json!({ "result": result })
        }
        Err(e) => json!({ "error": e }), // the connection may be broken: it is dropped, not put back
    }
}
