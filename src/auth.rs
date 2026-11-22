//! OAuth login for remote MCP servers, following the MCP authorization spec (2026-07-28).
//!
//! The steps, in order:
//!   1. The server says "401, you need a token" and points at its resource metadata.
//!   2. That metadata names the authorization server (the place that does the login).
//!   3. The authorization server's metadata says where to log in and where to get tokens.
//!   4. We get a client ID (from the config, a hosted metadata document, or by registering).
//!   5. The user logs in through the browser, and the browser comes back to us with a code.
//!   6. We swap the code for tokens and save them.

use crate::config;
use crate::util::{base64, base64url, now, random_token, sha256};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use url::Url;

/// What a server tells us in its `WWW-Authenticate: Bearer ...` header.
#[derive(Default, Debug)]
pub struct Challenge {
    pub resource_metadata: Option<String>,
    pub scope: Option<String>,
    pub error: Option<String>,
}

pub fn parse_challenge(header: Option<&str>) -> Challenge {
    let mut out = Challenge::default();
    let Some(header) = header else { return out };
    let text = header.trim();
    let text = if text.len() >= 6 && text[..6].eq_ignore_ascii_case("bearer") { &text[6..] } else { text };
    // Read key=value pairs. Values may be in quotes, and quoted values may hold commas.
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && (chars[i] == ',' || chars[i].is_whitespace()) {
            i += 1;
        }
        let start = i;
        while i < chars.len() && chars[i] != '=' && chars[i] != ',' {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect::<String>().trim().to_ascii_lowercase();
        if i >= chars.len() || chars[i] != '=' {
            continue;
        }
        i += 1;
        let mut value = String::new();
        if i < chars.len() && chars[i] == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                value.push(chars[i]);
                i += 1;
            }
            i += 1;
        } else {
            while i < chars.len() && chars[i] != ',' && !chars[i].is_whitespace() {
                value.push(chars[i]);
                i += 1;
            }
        }
        match key.as_str() {
            "resource_metadata" => out.resource_metadata = Some(value),
            "scope" => out.scope = Some(value),
            "error" => out.error = Some(value),
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// The saved logins: ~/.config/dogu/auth.json, readable only by you.
//   tokens:  one entry per MCP server URL
//   clients: one entry per authorization server (a client ID belongs to the server that issued it)

fn store_path() -> PathBuf {
    match std::env::var("DOGU_AUTH_FILE") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => config::home().join(".config/dogu/auth.json"),
    }
}

fn load_store() -> Value {
    std::fs::read_to_string(store_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({ "tokens": {}, "clients": {} }))
}

fn save_store(store: &Value) -> Result<(), String> {
    let path = store_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // Write to a new file and move it into place, so a crash cannot leave half a file.
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        file.write_all(serde_json::to_string_pretty(store).unwrap().as_bytes()).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Forget the saved login for a server.
pub fn logout(server_url: &str) -> Result<bool, String> {
    let mut store = load_store();
    let had = store["tokens"].as_object_mut().map_or(false, |t| t.remove(server_url).is_some());
    save_store(&store)?;
    Ok(had)
}

// ---------------------------------------------------------------------------------------

/// The login state for one HTTP server.
pub struct Auth {
    name: String,
    server_url: String,
    /// A client ID from the config, for servers where you registered an app yourself.
    preregistered: Option<(String, Option<String>)>,
    /// May we open a browser and ask the user to log in?
    pub interactive: bool,
    attempts: u32,
    refreshed: bool,
}

impl Auth {
    pub fn new(name: &str, server_url: &str, oauth: &Value, interactive: bool) -> Auth {
        let preregistered = oauth["client_id"]
            .as_str()
            .map(|id| (id.to_string(), oauth["client_secret"].as_str().map(String::from)));
        Auth {
            name: name.to_string(),
            server_url: server_url.to_string(),
            preregistered,
            interactive,
            attempts: 0,
            refreshed: false,
        }
    }

    /// The access token to send, if we have one. Refreshes it first when it is about to expire.
    pub fn bearer(&mut self) -> Option<String> {
        let record = load_store()["tokens"][&self.server_url].clone();
        let expires = record["expires_at"].as_u64().unwrap_or(0);
        if expires != 0 && expires <= now() + 30 && record["refresh_token"].is_string() {
            let _ = self.refresh();
            return load_store()["tokens"][&self.server_url]["access_token"].as_str().map(String::from);
        }
        record["access_token"].as_str().map(String::from)
    }

    /// A request worked: forget the failed attempts.
    pub fn ok(&mut self) {
        self.attempts = 0;
        self.refreshed = false;
    }

    /// The server said 401. Get a working token, or explain what the user must do.
    pub fn unauthorized(&mut self, www_authenticate: Option<&str>) -> Result<(), String> {
        self.attempts += 1;
        if self.attempts > 3 {
            return Err(format!("{} keeps refusing the login. Giving up.", self.name));
        }
        let has_refresh = load_store()["tokens"][&self.server_url]["refresh_token"].is_string();
        if has_refresh && !self.refreshed {
            self.refreshed = true;
            if self.refresh().is_ok() {
                return Ok(());
            }
        }
        if !self.interactive {
            return Err(format!("{0} needs a login. Run: dogu login {0}", self.name));
        }
        self.login(&parse_challenge(www_authenticate), false)
    }

    /// The server said 403. If it wants more permissions, log in again asking for them.
    /// Returns true if the request is worth trying again.
    pub fn forbidden(&mut self, www_authenticate: Option<&str>) -> Result<bool, String> {
        let challenge = parse_challenge(www_authenticate);
        if challenge.error.as_deref() != Some("insufficient_scope") {
            return Ok(false);
        }
        self.attempts += 1;
        if self.attempts > 3 {
            return Err(format!("{} keeps asking for more permissions. Giving up.", self.name));
        }
        if !self.interactive {
            return Err(format!("{0} needs more permissions for this. Run: dogu login {0}", self.name));
        }
        self.login(&challenge, true)?;
        Ok(true)
    }

    fn refresh(&mut self) -> Result<(), String> {
        let mut store = load_store();
        let record = store["tokens"][&self.server_url].clone();
        let refresh_token = record["refresh_token"].as_str().ok_or("no refresh token")?;
        let token_endpoint = record["token_endpoint"].as_str().ok_or("no token endpoint")?;
        let client = store["clients"][record["issuer"].as_str().unwrap_or("")].clone();
        if !client.is_object() {
            return Err("no client".into());
        }
        let mut form = vec![
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh_token.to_string()),
            ("resource", record["resource"].as_str().unwrap_or(&self.server_url).to_string()),
        ];
        let tokens = match token_request(token_endpoint, &client, &mut form) {
            Ok(t) => t,
            Err(e) => {
                // The refresh token is dead. Drop the login so the next step is a fresh one.
                if let Some(t) = store["tokens"].as_object_mut() {
                    t.remove(&self.server_url);
                }
                save_store(&store)?;
                return Err(e);
            }
        };
        let entry = &mut store["tokens"][&self.server_url];
        entry["access_token"] = tokens["access_token"].clone();
        if tokens["refresh_token"].is_string() {
            entry["refresh_token"] = tokens["refresh_token"].clone();
        }
        entry["expires_at"] = json!(tokens["expires_in"].as_u64().map_or(0, |s| now() + s));
        save_store(&store)
    }

    /// The full browser login.
    pub fn login(&mut self, challenge: &Challenge, step_up: bool) -> Result<(), String> {
        let server = Url::parse(&self.server_url).map_err(|e| format!("bad server URL: {e}"))?;
        let mut store = load_store();
        let previous = store["tokens"][&self.server_url].clone();

        // 1. Ask the MCP server which authorization server it trusts.
        let resource_doc = discover_resource(&server, challenge.resource_metadata.as_deref())?;
        let (issuer, resource, resource_scopes) = match &resource_doc {
            Some(doc) => {
                let resource = match doc["resource"].as_str() {
                    Some(r) => {
                        check_resource(&server, r)?;
                        r.to_string()
                    }
                    None => canonical(&server),
                };
                let issuer = doc["authorization_servers"][0]
                    .as_str()
                    .ok_or("the server's metadata names no authorization server")?;
                let scopes = doc["scopes_supported"].as_array().map(|a| join_scopes(a));
                (issuer.to_string(), resource, scopes)
            }
            // Servers from before 2025-06 have no such document: the server itself does the login.
            None => (server.origin().ascii_serialization(), canonical(&server), None),
        };

        // 2. Ask the authorization server where its login and token pages are.
        let meta = discover_authorization_server(&issuer, resource_doc.is_some())?;
        let authorization_endpoint = meta["authorization_endpoint"].as_str().ok_or("no authorization_endpoint")?;
        let token_endpoint = meta["token_endpoint"].as_str().ok_or("no token_endpoint")?.to_string();
        if meta["_guessed"] != true {
            let s256 = meta["code_challenge_methods_supported"]
                .as_array()
                .map_or(false, |m| m.iter().any(|v| v == "S256"));
            if !s256 {
                return Err(format!("{issuer} does not support PKCE (S256), so logging in would not be safe"));
            }
        }

        // 3. Which permissions to ask for: what the server asked for, else what it lists, else nothing.
        let mut scopes: Vec<String> = Vec::new();
        let mut add = |text: &str| {
            for s in text.split_whitespace() {
                if !scopes.iter().any(|have| have == s) {
                    scopes.push(s.to_string());
                }
            }
        };
        if step_up {
            add(previous["scope"].as_str().unwrap_or("")); // keep what we already had
        }
        match (&challenge.scope, &resource_scopes) {
            (Some(s), _) => add(s),
            (None, Some(s)) => add(s),
            _ => {}
        }
        let offers_offline =
            meta["scopes_supported"].as_array().map_or(false, |a| a.iter().any(|v| v == "offline_access"));
        if offers_offline {
            add("offline_access"); // lets the server give us a refresh token
        }
        let scope = scopes.join(" ");

        // 4. Where the browser comes back to.
        let headless = std::env::var("DOGU_AUTH_HEADLESS").is_ok();
        let fixed_port: u16 =
            std::env::var("DOGU_CALLBACK_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(38573);
        let (listener, port) = if headless {
            (None, fixed_port)
        } else {
            let listener = TcpListener::bind(("127.0.0.1", fixed_port))
                .or_else(|_| TcpListener::bind(("127.0.0.1", 0)))
                .map_err(|e| format!("cannot listen for the browser: {e}"))?;
            let port = listener.local_addr().map_err(|e| e.to_string())?.port();
            (Some(listener), port)
        };
        let redirect_uri = format!("http://127.0.0.1:{port}/callback");

        // 5. Get a client ID.
        let client = self.client_for(&mut store, &issuer, &meta, &redirect_uri, port == fixed_port)?;
        let client_id = client["client_id"].as_str().unwrap_or("").to_string();

        // 6. Send the user to the login page.
        let verifier = random_token(48);
        let state = random_token(24);
        let mut url = Url::parse(authorization_endpoint).map_err(|e| format!("bad authorization_endpoint: {e}"))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("response_type", "code")
                .append_pair("client_id", &client_id)
                .append_pair("redirect_uri", &redirect_uri)
                .append_pair("state", &state)
                .append_pair("code_challenge", &base64url(&sha256(verifier.as_bytes())))
                .append_pair("code_challenge_method", "S256")
                .append_pair("resource", &resource);
            if !scope.is_empty() {
                q.append_pair("scope", &scope);
            }
        }
        let answer = match listener {
            Some(listener) => {
                eprintln!("\nTo log in to {}, open this link:\n\n  {url}\n", self.name);
                open_browser(url.as_str());
                eprintln!("Waiting for you to finish in the browser...");
                wait_for_callback(listener, Duration::from_secs(300))?
            }
            None => headless_callback(url.as_str())?,
        };

        // 7. Check the answer really came from the authorization server we chose.
        //    This check comes first: if it fails we must not trust anything else in the answer.
        let expected_issuer = meta["issuer"].as_str().unwrap_or(&issuer);
        match answer.get("iss") {
            Some(iss) if iss != expected_issuer => {
                return Err("the login answer came from a different authorization server. Stopped.".into())
            }
            None if meta["authorization_response_iss_parameter_supported"] == true => {
                return Err("the login answer does not say which authorization server sent it. Stopped.".into())
            }
            _ => {}
        }
        if answer.get("state") != Some(&state) {
            return Err("the login answer does not match this login attempt. Stopped.".into());
        }
        if let Some(error) = answer.get("error") {
            let detail = answer.get("error_description").map(|d| format!(": {d}")).unwrap_or_default();
            return Err(format!("the login was refused ({error}{detail})"));
        }
        let code = answer.get("code").ok_or("the login answer has no code")?;

        // 8. Swap the code for tokens.
        let mut form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", code.clone()),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
            ("resource", resource.clone()),
        ];
        let tokens = token_request(&token_endpoint, &client, &mut form)?;
        let access = tokens["access_token"].as_str().ok_or("the token answer has no access_token")?;

        let mut store = load_store(); // read again: client_for may have saved a new client
        store["tokens"][&self.server_url] = json!({
            "issuer": expected_issuer,
            "resource": resource,
            "scope": scope,
            "token_endpoint": token_endpoint,
            "access_token": access,
            "refresh_token": tokens["refresh_token"],
            "expires_at": tokens["expires_in"].as_u64().map_or(0, |s| now() + s),
        });
        save_store(&store)?;
        self.refreshed = false;
        Ok(())
    }

    /// Find or make the client ID for this authorization server.
    /// Order: one from the config, a hosted metadata document, one we registered before, register now.
    fn client_for(
        &self,
        store: &mut Value,
        issuer: &str,
        meta: &Value,
        redirect_uri: &str,
        fixed_port: bool,
    ) -> Result<Value, String> {
        let methods: Vec<&str> = meta["token_endpoint_auth_methods_supported"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        // With a secret, prefer sending it in the header, then in the form.
        let secret_method = || {
            if methods.is_empty() || methods.contains(&"client_secret_basic") {
                "client_secret_basic"
            } else if methods.contains(&"client_secret_post") {
                "client_secret_post"
            } else {
                "client_secret_basic"
            }
        };

        if let Some((id, secret)) = &self.preregistered {
            let method = if secret.is_some() { secret_method() } else { "none" };
            let client = json!({ "client_id": id, "client_secret": secret, "method": method, "kind": "config" });
            store["clients"][issuer] = client.clone();
            save_store(store)?;
            return Ok(client);
        }

        let document = std::env::var("DOGU_CLIENT_ID_URL").ok().filter(|u| u.starts_with("https://"));
        if let Some(url) = document {
            if meta["client_id_metadata_document_supported"] == true && fixed_port {
                return Ok(json!({ "client_id": url, "method": "none", "kind": "document" }));
            }
        }

        let saved = &store["clients"][issuer];
        if saved["kind"] == "registered" && saved["redirect_uri"] == redirect_uri {
            return Ok(saved.clone());
        }

        let Some(endpoint) = meta["registration_endpoint"].as_str() else {
            return Err(format!(
                "{issuer} does not let apps register themselves. Register an app there yourself, then add \
                 \"oauth\": {{\"client_id\": \"...\"}} to \"{}\" in your config.",
                self.name
            ));
        };
        let ask_method = if methods.is_empty() || methods.contains(&"none") { "none" } else { secret_method() };
        let body = json!({
            "client_name": "dogu",
            "redirect_uris": [redirect_uri],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": ask_method,
            "application_type": "native",
        });
        let answer = agent(endpoint, true)
            .post(endpoint)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(|e| format!("could not register with {issuer}: {}", describe(e)))?;
        let answer: Value =
            serde_json::from_str(&answer.into_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let id = answer["client_id"].as_str().ok_or("registration gave no client_id")?;
        let secret = answer["client_secret"].as_str();
        let method = match answer["token_endpoint_auth_method"].as_str() {
            Some(m) => m,
            None if secret.is_some() => secret_method(),
            None => "none",
        };
        let client = json!({ "client_id": id, "client_secret": secret, "method": method,
                             "redirect_uri": redirect_uri, "kind": "registered" });
        store["clients"][issuer] = client.clone();
        save_store(store)?;
        Ok(client)
    }
}

// ---------------------------------------------------------------------------------------
// Discovery

fn discover_resource(server: &Url, from_header: Option<&str>) -> Result<Option<Value>, String> {
    if let Some(url) = from_header {
        return match get_json(url)? {
            Some(doc) => Ok(Some(doc)),
            None => Err(format!("the server points to {url} for its login details, but nothing is there")),
        };
    }
    let origin = server.origin().ascii_serialization();
    let path = server.path().trim_end_matches('/');
    let mut places = Vec::new();
    if !path.is_empty() {
        places.push(format!("{origin}/.well-known/oauth-protected-resource{path}"));
    }
    places.push(format!("{origin}/.well-known/oauth-protected-resource"));
    for url in places {
        if let Some(doc) = get_json(&url)? {
            return Ok(Some(doc));
        }
    }
    Ok(None)
}

/// The metadata must be about the server we are talking to, not some other one.
fn check_resource(server: &Url, resource: &str) -> Result<(), String> {
    let named = Url::parse(resource).map_err(|_| "the server's metadata has a bad resource URL".to_string())?;
    let ours = server.path().trim_end_matches('/');
    let theirs = named.path().trim_end_matches('/');
    let covers = ours == theirs || ours.starts_with(&format!("{theirs}/"));
    if named.origin() != server.origin() || !covers {
        return Err(format!(
            "the server's login details are for {resource}, not for {server}. Stopped, because a token for one \
             must not be sent to the other."
        ));
    }
    Ok(())
}

/// The server URL in the form the spec wants for `resource`: no fragment, no bare trailing slash.
fn canonical(server: &Url) -> String {
    let mut u = server.clone();
    u.set_fragment(None);
    let s = u.to_string();
    if u.path() == "/" && u.query().is_none() {
        s.trim_end_matches('/').to_string()
    } else {
        s
    }
}

fn discover_authorization_server(issuer: &str, strict: bool) -> Result<Value, String> {
    let parsed = Url::parse(issuer).map_err(|_| format!("bad authorization server URL: {issuer}"))?;
    let origin = parsed.origin().ascii_serialization();
    let path = parsed.path().trim_end_matches('/');
    let places = if path.is_empty() {
        vec![
            format!("{origin}/.well-known/oauth-authorization-server"),
            format!("{origin}/.well-known/openid-configuration"),
        ]
    } else {
        vec![
            format!("{origin}/.well-known/oauth-authorization-server{path}"),
            format!("{origin}/.well-known/openid-configuration{path}"),
            format!("{origin}{path}/.well-known/openid-configuration"),
        ]
    };
    let mut wrong_issuer = false;
    for url in places {
        let Some(doc) = get_json(&url)? else { continue };
        // The document must say it is about the issuer we asked for. If not, someone may be
        // pretending to be that issuer, and we must not use it.
        if doc["issuer"].as_str() == Some(issuer) {
            return Ok(doc);
        }
        wrong_issuer = true;
    }
    if wrong_issuer {
        return Err(format!("the metadata at {issuer} says it belongs to a different issuer. Stopped."));
    }
    if strict {
        return Err(format!("{issuer} has no authorization server metadata"));
    }
    // Very old servers (2025-03-26) may have no metadata at all. The spec of that time gave defaults.
    Ok(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{origin}/authorize"),
        "token_endpoint": format!("{origin}/token"),
        "registration_endpoint": format!("{origin}/register"),
        "_guessed": true,
    }))
}

fn join_scopes(list: &[Value]) -> String {
    list.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------------------
// HTTP

pub fn is_local(url: &str) -> bool {
    let host = url.split("://").nth(1).unwrap_or(url);
    ["localhost", "127.", "[::1]"].iter().any(|h| host.starts_with(h))
}

fn agent(url: &str, follow_redirects: bool) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .try_proxy_from_env(!is_local(url))
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .redirects(if follow_redirects { 5 } else { 0 })
        .build()
}

fn describe(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body: String = resp.into_string().unwrap_or_default().chars().take(300).collect();
            format!("HTTP {code} {body}")
        }
        other => other.to_string(),
    }
}

/// GET a JSON document. A 4xx answer means "not here" and gives None.
fn get_json(url: &str) -> Result<Option<Value>, String> {
    match agent(url, true).get(url).set("Accept", "application/json").call() {
        Ok(resp) => Ok(resp.into_string().ok().and_then(|t| serde_json::from_str(&t).ok())),
        Err(ureq::Error::Status(code, _)) if (400..500).contains(&code) => Ok(None),
        Err(e) => Err(format!("could not reach {url}: {}", describe(e))),
    }
}

/// POST to the token endpoint, proving who the client is in the way that client was set up.
fn token_request(endpoint: &str, client: &Value, form: &mut Vec<(&str, String)>) -> Result<Value, String> {
    let id = client["client_id"].as_str().unwrap_or("");
    let secret = client["client_secret"].as_str().unwrap_or("");
    let mut request = agent(endpoint, true)
        .post(endpoint)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Accept", "application/json");
    match client["method"].as_str() {
        Some("client_secret_basic") => {
            let enc = |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>();
            let pair = format!("{}:{}", enc(id), enc(secret));
            request = request.set("Authorization", &format!("Basic {}", base64(pair.as_bytes())));
        }
        Some("client_secret_post") => {
            form.push(("client_id", id.to_string()));
            form.push(("client_secret", secret.to_string()));
        }
        _ => form.push(("client_id", id.to_string())),
    }
    let body: String = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form.iter().map(|(k, v)| (*k, v.as_str())))
        .finish();
    let resp = request.send_string(&body).map_err(|e| format!("the token request failed: {}", describe(e)))?;
    serde_json::from_str(&resp.into_string().map_err(|e| e.to_string())?)
        .map_err(|_| "the token answer is not JSON".to_string())
}

// ---------------------------------------------------------------------------------------
// The browser round trip

fn open_browser(url: &str) {
    if std::env::var("DOGU_NO_BROWSER").is_ok() {
        return;
    }
    let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = std::process::Command::new(program)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Wait for the browser to come back to http://127.0.0.1:<port>/callback?code=...
fn wait_for_callback(listener: TcpListener, limit: Duration) -> Result<BTreeMap<String, String>, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + limit;
    loop {
        if Instant::now() > deadline {
            return Err("nobody finished the login in the browser in time".into());
        }
        let Ok((mut stream, _)) = listener.accept() else {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut first_line = String::new();
        if BufReader::new(&stream).read_line(&mut first_line).is_err() {
            continue;
        }
        // "GET /callback?code=...&state=... HTTP/1.1"
        let target = first_line.split_whitespace().nth(1).unwrap_or("");
        if !target.starts_with("/callback") {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        }
        let page = "<!doctype html><meta charset=utf-8><title>dogu</title>\
                    <body style=\"font:16px system-ui;margin:15vh auto;max-width:28em\">\
                    <h2>Done.</h2><p>You can close this tab and go back to the terminal.</p>";
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        return Ok(query_of(target));
    }
}

/// Test mode only (DOGU_AUTH_HEADLESS): fetch the login page ourselves and read where it sends us.
/// This works only with test servers that log in without asking a person anything.
fn headless_callback(url: &str) -> Result<BTreeMap<String, String>, String> {
    let resp = match agent(url, false).get(url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return Err(format!("could not open the login page: {e}")),
    };
    let location = resp.header("location").ok_or("the login page did not send the browser back")?;
    Ok(query_of(location))
}

fn query_of(target: &str) -> BTreeMap<String, String> {
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
    let query = query.split('#').next().unwrap_or("");
    url::form_urlencoded::parse(query.as_bytes()).map(|(k, v)| (k.into_owned(), v.into_owned())).collect()
}

/// For `dogu login`: is there a saved login for this server?
pub fn has_login(server_url: &str) -> bool {
    load_store()["tokens"][server_url]["access_token"].is_string()
}
