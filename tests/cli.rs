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
        // No background process unless a test asks for one: it would outlive the test.
        .env("DOGU_WARM", "0")
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

// --- A server that dies at start says why ---

#[test]
fn a_server_that_dies_at_start_shows_what_it_said() {
    let home = scratch("dies");
    let config = home.join("dies.json");
    let script = "echo 'npm error code EPERM' >&2; echo 'npm error path /Users/me/.npm/_cacache/tmp/x' >&2; exit 1";
    write_json(&config, &json!({ "mcpServers": { "dies": { "command": "sh", "args": ["-c", script] } } }));

    let o = dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", home.join("out").to_str().unwrap()]);

    assert!(stdout(&o).contains("npm error code EPERM"), "shows the server's own words: {}", stdout(&o));
    assert!(stdout(&o).contains("~/.npm"), "says what probably happened: {}", stdout(&o));
}

// --- What a tool may change, as the server says ---

#[test]
fn the_readme_says_what_a_tool_may_change_when_the_server_says() {
    let home = scratch("labels");
    let config = fixture_config(&home);
    let out = home.join("mcp");

    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);

    let readme = std::fs::read_to_string(out.join("README.md")).unwrap();
    let line = |tool: &str| readme.lines().find(|l| l.contains(&format!("`fx/{tool}"))).unwrap_or("").to_string();
    assert!(line("search_repositories").contains("[read-only]"), "{readme}");
    assert!(line("count").contains("[changes things]"), "{readme}");
    assert!(line("reset").contains("[may delete or overwrite]"), "{readme}");
    assert!(!line("wiki").contains('['), "no label when the server says nothing: {}", line("wiki"));
}

#[test]
fn read_only_sync_keeps_only_read_only_tools_and_remembers_it() {
    let home = scratch("read-only");
    let config = fixture_config(&home);
    let out = home.join("mcp");
    let sync = |extra: &[&str]| {
        let mut args = vec!["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()];
        args.extend_from_slice(extra);
        dogu(&home, &home, &args)
    };

    let first = sync(&["--read-only"]);
    let again = sync(&[]);

    assert!(stdout(&first).contains("left out 3 tools"), "{}", stdout(&first));
    assert!(out.join("fx/search_repositories").is_file());
    for gone in ["count", "reset", "wiki"] {
        assert!(!out.join("fx").join(gone).exists(), "{gone} should be left out");
    }
    assert!(stdout(&again).contains("read-only"), "a re-sync keeps it: {}", stdout(&again));
    assert!(!out.join("fx/count").exists());

    sync(&["--all-tools"]);
    assert!(out.join("fx/count").is_file(), "--all-tools brings them back");
}

#[test]
fn a_left_out_tool_cannot_be_called_from_the_folder_or_from_code() {
    let home = scratch("read-only-call");
    let config = fixture_config(&home);
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap(), "--read-only"]);
    let root = out.to_str().unwrap();

    let call = dogu(&home, &home, &["call", "--root", root, "fx", "reset"]);
    let code = dogu(&home, &home, &["run", "--root", root, "-e", "await tools.fx.reset({})"]);

    assert_eq!(call.status.code(), Some(1));
    assert!(stderr(&call).contains("read-only"), "{}", stderr(&call));
    assert_ne!(code.status.code(), Some(0), "{}", stdout(&code));
}

// --- Calls a program makes together run together ---

#[test]
fn calls_made_together_run_at_the_same_time() {
    let home = scratch("parallel");
    let config = fixture_config(&home);
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let program = "const r = await Promise.all([1, 2, 3, 4].map(() => tools.fx.slow({}))); console.log(r.length);";

    let started = std::time::Instant::now();
    let o = dogu(&home, &home, &["run", "--root", out.to_str().unwrap(), "-e", program]);
    let took = started.elapsed();

    assert_eq!(stdout(&o).trim(), "4", "stderr: {}", stderr(&o));
    // One after another, four 0.6 s calls take 2.4 s; together, about one call plus start-up.
    assert!(took < std::time::Duration::from_millis(1800), "took {took:?}");
}

#[test]
fn a_failed_call_shows_the_tools_type_so_the_next_try_can_be_right() {
    let home = scratch("error-type");
    let config = fixture_config(&home);
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let program = "try { await tools.fx.search_repositories({ q: 'x' }) } catch (e) { console.log(e.message) }";

    let o = dogu(&home, &home, &["run", "--root", out.to_str().unwrap(), "-e", program]);

    let text = stdout(&o);
    assert!(text.contains("query is required"), "{text}");
    assert!(text.contains("tools.fx.search_repositories(args: {") && text.contains("query: string;"), "{text}");
}

// --- dogu serve connects to every server as it starts, so the first program does not wait ---

#[test]
fn serve_connects_while_the_model_thinks_so_the_first_program_is_quick() {
    use std::io::{BufRead, BufReader, Write};
    let home = scratch("warm");
    let server = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_server.py");
    let config = home.join("slow.json");
    write_json(&config, &json!({ "mcpServers": { "fx": { "command": "python3", "args": [server], "env": { "FIXTURE_START_DELAY": "1" } } } }));
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let mut serve = Command::new(env!("CARGO_BIN_EXE_dogu"))
        .args(["serve", "--root", out.to_str().unwrap()])
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = serve.stdin.take().unwrap();
    let mut lines = BufReader::new(serve.stdout.take().unwrap()).lines();
    let mut ask = |msg: Value| {
        writeln!(stdin, "{msg}").unwrap();
        serde_json::from_str::<Value>(&lines.next().unwrap().unwrap()).unwrap()
    };
    ask(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } } }));
    std::thread::sleep(std::time::Duration::from_millis(1500)); // the model reads and thinks

    let started = std::time::Instant::now();
    let answer = ask(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "run_code", "arguments": { "code": "console.log((await tools.fx.count({})).count)" } } }));
    let took = started.elapsed();

    let _ = serve.kill();
    assert!(answer["result"]["content"][0]["text"].as_str().unwrap_or("").contains('1'), "{answer}");
    assert!(took < std::time::Duration::from_millis(600), "the first program waited {took:?} for the server to start");
}

// --- Calls to an HTTP server reuse one connection ---

#[test]
fn calls_to_an_http_server_reuse_one_connection() {
    use std::io::{BufRead, BufReader};
    let home = scratch("keep-alive");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/http_mcp_server.py");
    let mut server = Command::new("python3").arg(&script).stdout(Stdio::piped()).spawn().unwrap();
    let mut port = String::new();
    BufReader::new(server.stdout.take().unwrap()).read_line(&mut port).unwrap();
    let config = home.join("web.json");
    write_json(&config, &json!({ "mcpServers": { "web": { "url": format!("http://127.0.0.1:{}/mcp", port.trim()) } } }));
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let program = "for (let i = 0; i < 4; i++) await tools.web.ping({}); console.log(await tools.web.connections({}));";

    let o = dogu(&home, &home, &["run", "--root", out.to_str().unwrap(), "-e", program]);
    let started = std::time::Instant::now();
    let held = dogu(&home, &home, &["run", "--root", out.to_str().unwrap(), "-e", "console.log(await tools.web.hold({}))"]);
    let took = started.elapsed();
    let _ = server.kill();

    // Sync used its own connection; the run should add just one for all five calls.
    let seen: usize = stdout(&o).trim().parse().unwrap_or_else(|_| panic!("{} / {}", stdout(&o), stderr(&o)));
    assert!(seen <= 2, "the run opened {} connections for 5 calls", seen - 1);
    // A server that keeps its stream open after the answer must not hold the answer up.
    assert!(stdout(&held).contains("pong"), "{}", stderr(&held));
    assert!(took < std::time::Duration::from_millis(1500), "the answer waited {took:?} for the stream to end");
}

// --- A warm folder: a background dogu keeps servers running between `dogu call`s ---

/// `dogu call` with the background process on, and extra environment.
fn warm_call(home: &Path, root: &Path, tool: &str, env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(DOGU);
    c.args(["call", "--root", root.to_str().unwrap(), "fx", tool, "--json"])
        .env("HOME", home)
        .env_remove("DOGU_CONFIG")
        .env("DOGU_WARM_IDLE", "3")
        .stdin(Stdio::null());
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn wait_for(what: impl Fn() -> bool) -> bool {
    (0..60).any(|_| what() || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        false
    })
}

fn warm_folder(name: &str) -> (PathBuf, PathBuf) {
    let home = scratch(name);
    let server = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_server.py");
    let config = home.join("fx.json");
    // MARK is passed through to the server, so a different value means different settings.
    write_json(&config, &json!({ "mcpServers": { "fx": { "command": "python3", "args": [server], "env": { "MARK": "${MARK}" } } } }));
    let out = home.join("mcp");
    let mut sync = Command::new(DOGU);
    sync.args(["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()])
        .env("HOME", &home)
        .env("MARK", "a")
        .env("DOGU_WARM", "0");
    sync.output().unwrap();
    (home, out)
}

#[test]
fn the_second_call_finds_the_server_still_running() {
    let (home, root) = warm_folder("warm-reuse");
    let sock = root.join(".dogu/warm.json");

    // The first call runs directly, with a server of its own, and starts the background process.
    let first = warm_call(&home, &root, "count", &[("MARK", "a")]);
    assert!(wait_for(|| sock.exists()), "no background process started");
    let second = warm_call(&home, &root, "count", &[("MARK", "a")]);
    let third = warm_call(&home, &root, "count", &[("MARK", "a")]);

    assert_eq!(stdout(&first).trim(), r#"{"count":1}"#, "{}", stderr(&first));
    assert_eq!(stdout(&second).trim(), r#"{"count":1}"#, "{}", stderr(&second));
    assert_eq!(stdout(&third).trim(), r#"{"count":2}"#, "the third call did not reach the server the second one used");
}

#[test]
fn different_settings_never_reach_a_server_started_with_others() {
    let (home, root) = warm_folder("warm-settings");
    let sock = root.join(".dogu/warm.json");
    warm_call(&home, &root, "count", &[("MARK", "a")]);
    assert!(wait_for(|| sock.exists()));

    let other = warm_call(&home, &root, "count", &[("MARK", "b")]);

    assert_eq!(stdout(&other).trim(), r#"{"count":1}"#, "a call with other settings used the old server");
    assert!(wait_for(|| !sock.exists()), "the old background process did not step aside");
}

#[test]
fn a_new_sync_retires_the_background_process() {
    let (home, root) = warm_folder("warm-resync");
    let sock = root.join(".dogu/warm.json");
    warm_call(&home, &root, "count", &[("MARK", "a")]);
    assert!(wait_for(|| sock.exists()));

    std::thread::sleep(std::time::Duration::from_millis(1100)); // so the sync gets a new time
    let config = home.join("fx.json");
    let mut sync = Command::new(DOGU);
    sync.args(["sync", "--config", config.to_str().unwrap(), "--out", root.to_str().unwrap()])
        .env("HOME", &home)
        .env("MARK", "a")
        .env("DOGU_WARM", "0");
    sync.output().unwrap();

    assert!(wait_for(|| !sock.exists()), "the background process outlived a new sync");
    let after = warm_call(&home, &root, "count", &[("MARK", "a")]);
    assert_eq!(stdout(&after).trim(), r#"{"count":1}"#, "{}", stderr(&after));
}

#[test]
fn the_background_process_leaves_when_idle() {
    let (home, root) = warm_folder("warm-idle");
    let sock = root.join(".dogu/warm.json");
    warm_call(&home, &root, "count", &[("MARK", "a")]);
    assert!(wait_for(|| sock.exists()));

    let gone = (0..100).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        !sock.exists()
    });

    assert!(gone, "still running after the idle time");
}

// --- A repeat of the same call to a read-only tool, soon after, is answered from memory ---

#[test]
fn a_repeated_read_only_call_is_answered_at_once_and_others_are_not() {
    let home = scratch("read-cache");
    let config = fixture_config(&home);
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let program = "const t = async (label) => (await tools.fx.tick({ label })).tick;
        console.log(await t('a'), await t('a'), await t('b'));
        console.log((await tools.fx.count({})).count, (await tools.fx.count({})).count);";

    let o = dogu(&home, &home, &["run", "--root", out.to_str().unwrap(), "-e", program]);

    let lines: Vec<String> = stdout(&o).lines().map(String::from).collect();
    assert_eq!(lines.first().map(String::as_str), Some("1 1 2"), "same call, same answer; other arguments, a new call: {}", stderr(&o));
    assert_eq!(lines.get(1).map(String::as_str), Some("1 2"), "a tool that changes things is never cached");
}

#[test]
fn the_read_cache_can_be_turned_off() {
    // How long answers are kept is tested in cache.rs, with the clock passed in.
    let home = scratch("read-cache-off");
    let config = fixture_config(&home);
    let out = home.join("mcp");
    dogu(&home, &home, &["sync", "--config", config.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let run = |env: &[(&str, &str)], code: &str| {
        let mut c = Command::new(DOGU);
        c.args(["run", "--root", out.to_str().unwrap(), "-e", code]).env("HOME", &home).env("DOGU_WARM", "0");
        for (k, v) in env {
            c.env(k, v);
        }
        c.output().unwrap()
    };

    let off = run(&[("DOGU_CACHE_SECS", "0")], "const t = async () => (await tools.fx.tick({})).tick; console.log(await t(), await t());");

    assert_eq!(stdout(&off).trim(), "1 2", "DOGU_CACHE_SECS=0 turns the cache off: {}", stderr(&off));
}

#[test]
fn the_warm_folder_answers_a_repeated_read_only_command_from_memory() {
    let (home, root) = warm_folder("warm-cache");
    let sock = root.join(".dogu/warm.json");
    warm_call(&home, &root, "count", &[("MARK", "a")]); // starts the background dogu
    assert!(wait_for(|| sock.exists()));

    let first = warm_call(&home, &root, "tick", &[("MARK", "a")]);
    let again = warm_call(&home, &root, "tick", &[("MARK", "a")]);

    assert_eq!(stdout(&first).trim(), stdout(&again).trim(), "{} / {}", stderr(&first), stderr(&again));
}
