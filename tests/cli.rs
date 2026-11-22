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

// --- Fewer turns: what the agent can see without asking ---

fn fixture_config(home: &Path) -> PathBuf {
    let server = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_server.py");
    let config = home.join("fixture.json");
    write_json(&config, &json!({ "mcpServers": { "fx": { "command": "python3", "args": [server] } } }));
    config
}

#[test]
fn the_top_readme_lists_every_tool_with_its_flags() {
    let home = scratch("index");
    let config = fixture_config(&home);
    let out = home.join("mcp");

    let o = dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);

    assert!(stdout(&o).contains("ok    fx"), "sync failed: {}{}", stdout(&o), stderr(&o));
    let readme = std::fs::read_to_string(out.join("README.md")).unwrap();
    assert!(readme.contains("`fx/search_repositories --query <string> [--sort stars|forks] [--perPage <number>]`"), "{readme}");
    assert!(readme.contains("`fx/count`"), "{readme}");
    assert!(readme.contains("Search GitHub repositories by a query."), "{readme}");
    assert!(readme.contains("→ {query, items}"), "the fields it returns: {readme}");
    assert!(readme.contains("`--json` prints JSON with those fields"), "says what the arrow means: {readme}");
}

#[test]
fn an_app_readme_lists_each_tool_with_its_flags() {
    let home = scratch("app-index");
    let config = fixture_config(&home);
    let out = home.join("mcp");

    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);

    let readme = std::fs::read_to_string(out.join("fx/README.md")).unwrap();
    assert!(readme.contains("`search_repositories --query <string> [--sort stars|forks] [--perPage <number>]`"), "{readme}");
}

#[test]
fn json_flag_prints_the_structured_result() {
    let home = scratch("json-flag");
    let config = fixture_config(&home);
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let root = out.to_str().unwrap();

    let text = dogu(&home, &home, &["call", "--root", root, "fx", "wiki"]);
    let json = dogu(&home, &home, &["call", "--root", root, "fx", "wiki", "--json"]);
    let search = dogu(&home, &home, &["call", "--root", root, "fx", "search_repositories", "--query", "x", "--json"]);

    assert_eq!(stdout(&text).trim(), "# Title\n- one\n- two");
    assert_eq!(stdout(&json).trim(), r##"{"result":"# Title\n- one\n- two"}"##);
    assert!(stdout(&search).trim().starts_with(r#"{"query":"x","items":[{"name":"servers""#), "{}", stdout(&search));
}

// --- Forge: agents save multi-step programs as new tools ---

const TWICE: &str = r#"// @description Count twice and report the total.
// @param label string required A name for the result
const a = await tools.fx.count({});
const b = await tools.fx.count({});
console.log(JSON.stringify({ label: args.label, last: b.count }));
"#;

/// A synced fixture folder with the forged tool `twice` saved in it. Returns (home, folder).
fn forged_folder(name: &str) -> (PathBuf, PathBuf) {
    let home = scratch(name);
    let config = fixture_config(&home);
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    std::fs::write(home.join("twice.ts"), TWICE).unwrap();
    let o = dogu(&home, &home, &["forge", "--root", out.to_str().unwrap(), "twice", home.join("twice.ts").to_str().unwrap()]);
    assert!(o.status.success(), "forge failed: {}{}", stdout(&o), stderr(&o));
    (home, out)
}

#[test]
fn forge_saves_a_tool_with_its_own_help_and_readme_line() {
    let (home, out) = forged_folder("forge-save");

    let help = Command::new(out.join("forged/twice")).arg("--help").env("HOME", &home).output().unwrap();
    let readme = std::fs::read_to_string(out.join("README.md")).unwrap();

    assert!(stdout(&help).contains("Count twice and report the total."), "{}", stdout(&help));
    assert!(stdout(&help).contains("--label <string>"), "{}", stdout(&help));
    assert!(readme.contains("`forged/twice --label <string>`"), "{readme}");
}

#[test]
fn a_forged_tool_runs_its_program_with_the_flags_as_args() {
    let (home, out) = forged_folder("forge-run");

    let o = dogu(&home, &home, &["call", "--root", out.to_str().unwrap(), "forged", "twice", "--label", "x"]);

    // Both calls in one run share a connection, so the server's counter reaches 2.
    assert_eq!(stdout(&o).trim(), r#"{"label":"x","last":2}"#, "stderr: {}", stderr(&o));
}

#[test]
fn a_forged_tool_checks_its_flags_like_any_tool() {
    let (home, out) = forged_folder("forge-flags");

    let o = dogu(&home, &home, &["call", "--root", out.to_str().unwrap(), "forged", "twice"]);

    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("missing --label"), "{}", stderr(&o));
}

#[test]
fn a_forged_tool_may_only_call_the_tools_its_code_names() {
    let home = scratch("forge-allow");
    let config = fixture_config(&home);
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let sneaky = "// @description Calls a tool it does not name.\nconst t = \"count\";\nconsole.log(await tools.fx[t]({}));\n";
    std::fs::write(home.join("sneaky.ts"), sneaky).unwrap();
    dogu(&home, &home, &["forge", "--root", out.to_str().unwrap(), "sneaky", home.join("sneaky.ts").to_str().unwrap()]);

    let o = dogu(&home, &home, &["call", "--root", out.to_str().unwrap(), "forged", "sneaky"]);

    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("not allowed"), "{}{}", stdout(&o), stderr(&o));
}

#[test]
fn sync_keeps_forged_tools() {
    let (home, out) = forged_folder("forge-resync");
    let config = home.join("fixture.json");

    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);

    assert!(out.join("forged/twice").is_file());
    assert!(std::fs::read_to_string(out.join("README.md")).unwrap().contains("`forged/twice --label <string>`"));
}

#[test]
fn code_mode_can_call_a_forged_tool() {
    let (home, out) = forged_folder("forge-code");

    let o = dogu(&home, &home, &["run", "--root", out.to_str().unwrap(), "-e", "const r = await tools.forged.twice({ label: 'y' }); console.log(r.label, r.last);"]);

    assert_eq!(stdout(&o).trim(), "y 2", "stderr: {}", stderr(&o));
}

#[test]
fn forge_rejects_bad_names_missing_descriptions_and_silent_replacing() {
    let (home, out) = forged_folder("forge-reject");
    let root = out.to_str().unwrap();
    let file = home.join("twice.ts");
    std::fs::write(home.join("plain.ts"), "console.log(1);\n").unwrap();

    let bad_name = dogu(&home, &home, &["forge", "--root", root, "Bad Name", file.to_str().unwrap()]);
    let no_description = dogu(&home, &home, &["forge", "--root", root, "plain", home.join("plain.ts").to_str().unwrap()]);
    let again = dogu(&home, &home, &["forge", "--root", root, "twice", file.to_str().unwrap()]);
    let forced = dogu(&home, &home, &["forge", "--root", root, "--force", "twice", file.to_str().unwrap()]);

    assert!(stderr(&bad_name).contains("name"), "{}", stderr(&bad_name));
    assert!(stderr(&no_description).contains("@description"), "{}", stderr(&no_description));
    assert!(stderr(&again).contains("--force"), "{}", stderr(&again));
    assert!(forced.status.success(), "{}", stderr(&forced));
}

// --- Agent Skills: the folder is a skill ---

#[test]
fn sync_writes_a_skill_file_that_says_what_the_tools_are_for() {
    let home = scratch("skill");
    let config = fixture_config(&home);
    let out = home.join("mcp");

    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);

    let skill = std::fs::read_to_string(out.join("SKILL.md")).unwrap();
    let frontmatter = skill.split("---").nth(1).expect("frontmatter");
    assert!(frontmatter.contains("name: mcp-tools"), "{skill}");
    let description = frontmatter.lines().find(|l| l.starts_with("description: ")).expect("description");
    assert!(description.contains("fx"), "names the apps: {description}");
    assert!(description.len() <= 1024, "the spec's limit: {}", description.len());
    assert!(skill.contains("README.md"), "{skill}");
    // The front matter must be valid YAML: the description has ": " in it, so it has to be quoted.
    let yaml = Command::new("ruby")
        .args(["-ryaml", "-e", "y = YAML.safe_load(STDIN.read.split('---')[1]); puts y['name']; puts y['description']"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(skill.as_bytes())?;
            c.wait_with_output()
        });
    if let Ok(out) = yaml {
        assert!(out.status.success(), "front matter is not valid YAML: {skill}");
        assert!(String::from_utf8_lossy(&out.stdout).starts_with("mcp-tools\nThe user's MCP tools"), "{}", String::from_utf8_lossy(&out.stdout));
    }
}

#[test]
fn link_skill_links_the_folder_into_the_agent_skill_folders() {
    let home = scratch("link-skill");
    let config = fixture_config(&home);
    let out = home.join("mcp");

    let o = dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap(), "--link-skill"]);

    for dir in [".claude/skills/mcp-tools", ".agents/skills/mcp-tools"] {
        let link = home.join(dir);
        assert!(link.join("SKILL.md").is_file(), "{dir} missing: {}{}", stdout(&o), stderr(&o));
    }
}
