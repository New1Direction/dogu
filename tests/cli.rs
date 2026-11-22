//! Tests that run the real `dogu` binary with a fake home folder.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const DOGU: &str = env!("CARGO_BIN_EXE_dogu");

/// A fresh, empty folder for one test. Its path has no symlinks, so it matches what
/// the binary sees as its current folder.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dogu-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(&dir).unwrap()
}

/// Run dogu with `home` as $HOME, in the folder `cwd`, with nothing on stdin.
fn dogu(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(DOGU)
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env_remove("DOGU_CONFIG")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn write_json(path: &Path, value: &Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

/// A local server that exits at once. Sync reports it as failed but still records it.
fn broken_server() -> Value {
    json!({ "command": "false" })
}

fn state_of(out: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(out.join(".dogu/state.json")).unwrap()).unwrap()
}

// --- --help and unknown commands ---

#[test]
fn every_subcommand_prints_help_and_exits_zero() {
    let home = scratch("help");
    for sub in ["sync", "login", "logout", "call", "run", "serve"] {
        for flag in ["--help", "-h"] {
            let o = dogu(&home, &home, &[sub, flag]);
            assert!(o.status.success(), "dogu {sub} {flag} failed: {}", stderr(&o));
            assert!(stdout(&o).contains(&format!("dogu {sub}")), "dogu {sub} {flag} printed: {}", stdout(&o));
            assert!(!stderr(&o).contains("error"), "dogu {sub} {flag} wrote an error: {}", stderr(&o));
        }
    }
}

#[test]
fn top_level_help_exits_zero() {
    let home = scratch("top-help");
    for args in [&["--help"][..], &["-h"], &["help"], &[]] {
        let o = dogu(&home, &home, args);
        assert!(o.status.success(), "dogu {args:?} failed");
        assert!(stdout(&o).contains("dogu sync"), "dogu {args:?} printed: {}", stdout(&o));
    }
}

#[test]
fn unknown_command_is_an_error() {
    let home = scratch("unknown");
    let o = dogu(&home, &home, &["snyc"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("unknown command snyc"), "stderr: {}", stderr(&o));
}

// --- Finding Claude Code's ~/.claude.json ---

#[test]
fn sync_reads_claude_code_user_servers() {
    let home = scratch("cc-user");
    write_json(&home.join(".claude.json"), &json!({ "numStartups": 3, "mcpServers": { "mine": broken_server() } }));
    let out = home.join("out");

    let o = dogu(&home, &home, &["sync", "--out", out.to_str().unwrap()]);

    assert!(stdout(&o).contains(".claude.json"), "stdout: {}", stdout(&o));
    assert!(stdout(&o).contains("FAIL  mine"), "stdout: {}", stdout(&o));
    assert_eq!(state_of(&out)["servers"], json!(["mine"]));
}

#[test]
fn sync_adds_this_projects_claude_code_servers() {
    let home = scratch("cc-project");
    let project = home.join("app");
    std::fs::create_dir_all(&project).unwrap();
    let key = project.to_str().unwrap();
    write_json(
        &home.join(".claude.json"),
        &json!({
            "mcpServers": { "everywhere": broken_server() },
            "projects": {
                key: { "mcpServers": { "here": broken_server() } },
                "/some/other/project": { "mcpServers": { "elsewhere": broken_server() } }
            }
        }),
    );
    let out = home.join("out");

    let o = dogu(&home, &project, &["sync", "--out", out.to_str().unwrap()]);

    let state = state_of(&out);
    assert_eq!(state["servers"], json!(["everywhere", "here"]), "stdout: {}", stdout(&o));
    assert_eq!(state["project"], json!(key));
}

#[test]
fn a_tool_call_finds_project_servers_from_any_folder() {
    let home = scratch("cc-call");
    let project = home.join("app");
    std::fs::create_dir_all(&project).unwrap();
    write_json(
        &home.join(".claude.json"),
        &json!({ "projects": { project.to_str().unwrap(): { "mcpServers": { "here": broken_server() } } } }),
    );
    let out = home.join("out");
    dogu(&home, &project, &["sync", "--out", out.to_str().unwrap()]);

    // Called from the home folder, not the project, the way an agent would.
    let o = dogu(&home, &home, &["call", "--root", out.to_str().unwrap(), "here", "anything"]);

    assert!(!stderr(&o).contains("is not in"), "the server was lost: {}", stderr(&o));
    assert!(stderr(&o).contains("exited"), "the server was not started: {}", stderr(&o));
}

#[test]
fn a_claude_json_with_no_servers_is_skipped() {
    let home = scratch("cc-empty");
    write_json(&home.join(".claude.json"), &json!({ "mcpServers": {}, "projects": { "/x": { "mcpServers": {} } } }));
    write_json(
        &home.join(".cursor/mcp.json"),
        &json!({ "mcpServers": { "from-cursor": broken_server() } }),
    );
    let out = home.join("out");

    let o = dogu(&home, &home, &["sync", "--out", out.to_str().unwrap()]);

    assert!(stdout(&o).contains(".cursor/mcp.json"), "stdout: {}", stdout(&o));
    assert_eq!(state_of(&out)["servers"], json!(["from-cursor"]));
}

#[test]
fn dogu_config_comes_before_claude_code() {
    let home = scratch("cc-order");
    write_json(&home.join(".claude.json"), &json!({ "mcpServers": { "cc": broken_server() } }));
    write_json(&home.join(".config/dogu/mcp.json"), &json!({ "mcpServers": { "dogu-own": broken_server() } }));
    let out = home.join("out");

    dogu(&home, &home, &["sync", "--out", out.to_str().unwrap()]);

    assert_eq!(state_of(&out)["servers"], json!(["dogu-own"]));
}

#[test]
fn claude_code_comes_before_claude_desktop() {
    let home = scratch("cc-desktop");
    write_json(&home.join(".claude.json"), &json!({ "mcpServers": { "cc": broken_server() } }));
    write_json(
        &home.join("Library/Application Support/Claude/claude_desktop_config.json"),
        &json!({ "mcpServers": { "desktop": broken_server() } }),
    );
    write_json(
        &home.join(".config/Claude/claude_desktop_config.json"),
        &json!({ "mcpServers": { "desktop": broken_server() } }),
    );
    let out = home.join("out");

    dogu(&home, &home, &["sync", "--out", out.to_str().unwrap()]);

    assert_eq!(state_of(&out)["servers"], json!(["cc"]));
}

#[test]
fn explicit_claude_json_also_gets_project_servers() {
    let home = scratch("cc-explicit");
    let project = home.join("app");
    std::fs::create_dir_all(&project).unwrap();
    let config = home.join(".claude.json");
    write_json(
        &config,
        &json!({
            "mcpServers": { "everywhere": broken_server() },
            "projects": { project.to_str().unwrap(): { "mcpServers": { "here": broken_server() } } }
        }),
    );
    let out = home.join("out");

    dogu(&home, &project, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);

    assert_eq!(state_of(&out)["servers"], json!(["everywhere", "here"]));
}

// --- Edge cases from review ---

#[test]
fn a_claude_json_that_only_lists_dogu_serve_is_skipped() {
    let home = scratch("cc-serve-only");
    write_json(&home.join(".claude.json"), &json!({ "mcpServers": { "tools": { "command": "dogu", "args": ["serve"] } } }));
    write_json(&home.join(".cursor/mcp.json"), &json!({ "mcpServers": { "from-cursor": broken_server() } }));
    let out = home.join("out");

    dogu(&home, &home, &["sync", "--out", out.to_str().unwrap()]);

    assert_eq!(state_of(&out)["servers"], json!(["from-cursor"]));
}

#[test]
fn resync_keeps_the_config_the_folder_was_made_from() {
    let home = scratch("resync");
    write_json(&home.join(".cursor/mcp.json"), &json!({ "mcpServers": { "from-cursor": broken_server() } }));
    let out = home.join("out");
    dogu(&home, &home, &["sync", "--out", out.to_str().unwrap()]);
    // Later the user starts using Claude Code, which comes earlier in the search order.
    write_json(&home.join(".claude.json"), &json!({ "mcpServers": { "cc": broken_server() } }));

    let o = dogu(&home, &home, &["sync", "--out", out.to_str().unwrap()]);

    assert_eq!(state_of(&out)["servers"], json!(["from-cursor"]), "stdout: {}", stdout(&o));
    assert!(stdout(&o).contains("--config"), "should say how to switch: {}", stdout(&o));
}

#[test]
fn an_empty_project_mcp_json_is_not_skipped() {
    let home = scratch("empty-local");
    write_json(&home.join(".mcp.json"), &json!({ "mcpServers": {} }));
    write_json(&home.join(".cursor/mcp.json"), &json!({ "mcpServers": { "from-cursor": broken_server() } }));
    let out = home.join("out");

    let o = dogu(&home, &home, &["sync", "--out", out.to_str().unwrap()]);

    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("lists no servers"), "stderr: {}", stderr(&o));
}

#[test]
fn a_config_with_no_servers_key_says_so() {
    let home = scratch("no-key");
    let config = home.join("odd.json");
    write_json(&config, &json!({ "something": "else" }));

    let o = dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", home.join("out").to_str().unwrap()]);

    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains(r#"has no "mcpServers""#), "stderr: {}", stderr(&o));
}

#[test]
fn a_projects_own_server_wins_over_the_users_server_of_the_same_name() {
    let home = scratch("cc-precedence");
    let project = home.join("app");
    std::fs::create_dir_all(&project).unwrap();
    write_json(
        &home.join(".claude.json"),
        &json!({
            "mcpServers": { "x": broken_server() },
            "projects": { project.to_str().unwrap(): { "mcpServers": { "x": { "url": "http://127.0.0.1:9/mcp" } } } }
        }),
    );

    let o = dogu(&home, &project, &["sync", "--out", home.join("out").to_str().unwrap()]);

    let line = stdout(&o).lines().find(|l| l.contains("FAIL  x")).map(String::from).unwrap_or_default();
    assert!(!line.is_empty() && !line.contains("exited"), "the user's stdio server was used: {}", stdout(&o));
}

#[test]
fn a_folder_synced_before_projects_were_saved_still_works() {
    let home = scratch("old-state");
    let config = home.join("mcp.json");
    write_json(&config, &json!({ "mcpServers": { "old": broken_server() } }));
    let out = home.join("out");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let old_state = json!({ "config": config, "servers": ["old"], "eras": {} });
    write_json(&out.join(".dogu/state.json"), &old_state);

    let o = dogu(&home, &home, &["call", "--root", out.to_str().unwrap(), "old", "anything"]);

    assert!(stderr(&o).contains("exited"), "stderr: {}", stderr(&o));
}

#[test]
fn call_help_after_root_prints_usage() {
    let home = scratch("call-root-help");
    let o = dogu(&home, &home, &["call", "--root", home.to_str().unwrap(), "--help"]);

    assert!(o.status.success(), "stderr: {}", stderr(&o));
    assert!(stdout(&o).contains("dogu call"), "stdout: {}", stdout(&o));
}

#[test]
fn help_after_a_tool_name_is_the_tools_own_help() {
    let home = scratch("tool-help");
    let o = dogu(&home, &home, &["call", "--root", home.to_str().unwrap(), "app", "some_tool", "--help"]);

    assert!(o.status.success(), "stderr: {}", stderr(&o));
    assert!(stdout(&o).starts_with("some_tool"), "stdout: {}", stdout(&o));
    assert!(!stdout(&o).contains("dogu login"), "printed dogu's usage: {}", stdout(&o));
}

#[test]
fn sync_shows_paths_under_home_with_a_tilde() {
    let home = scratch("tilde");
    write_json(&home.join(".claude.json"), &json!({ "mcpServers": { "mine": broken_server() } }));

    let o = dogu(&home, &home, &["sync", "--out", home.join("mcp").to_str().unwrap()]);

    assert!(stdout(&o).contains("Reading ~/.claude.json"), "stdout: {}", stdout(&o));
    assert!(stdout(&o).contains("Your tools are in ~/mcp. Read ~/mcp/README.md first."), "stdout: {}", stdout(&o));
    assert!(!stdout(&o).contains(home.to_str().unwrap()), "stdout: {}", stdout(&o));
}
