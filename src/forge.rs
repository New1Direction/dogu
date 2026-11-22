//! `dogu forge`: an agent saves a program it wrote as a new tool.
//!
//! The program is TypeScript for the sandbox, with a header that says what the tool is:
//!
//!   // @description Latest release tag of each of an org's most-starred repos.
//!   // @param org string required The GitHub org
//!   // @param top number How many repos
//!   const top = args.top ?? 5;
//!   ...
//!   console.log(JSON.stringify(result));
//!
//! It becomes `forged/<name>` in the folder and `tools.forged.<name>` in code. It runs in the
//! sandbox and may only call the tools its code names, so a forged tool can do no more than it shows.

use crate::run::{ident, run_program, Options, Pool};
use crate::sync::{saved_schemas, schema_path, tool_stub, types_file, write_root_readme, FORGED};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

const TYPES: [&str; 5] = ["string", "number", "integer", "boolean", "json"];
const MAX_NAME: usize = 64;

/// What a program's header and code say about the tool.
struct Header {
    description: String,
    params: Vec<Param>,
}

struct Param {
    name: String,
    kind: String,
    required: bool,
    about: String,
}

fn check_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= MAX_NAME
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(format!("\"{name}\" cannot be a tool name: use lowercase letters, digits and _, starting with a letter"))
    }
}

fn parse_header(source: &str) -> Result<Header, String> {
    let mut description = None;
    let mut params = Vec::new();
    for line in source.lines().map(str::trim) {
        if let Some(text) = line.strip_prefix("// @description") {
            description = Some(text.trim().to_string()).filter(|t| !t.is_empty());
        } else if let Some(text) = line.strip_prefix("// @param") {
            params.push(parse_param(text.trim())?);
        }
    }
    let description = description.ok_or("start the program with a `// @description <what the tool does>` line")?;
    Ok(Header { description, params })
}

/// `org string required The GitHub org` → a parameter.
fn parse_param(text: &str) -> Result<Param, String> {
    let mut words = text.splitn(3, ' ');
    let name = words.next().unwrap_or("").to_string();
    let kind = words.next().unwrap_or("").to_string();
    let rest = words.next().unwrap_or("").trim();
    if name.is_empty() || ident(&name) != name {
        return Err(format!("bad @param \"{text}\": write `// @param <name> <type> [required] <what it is>`"));
    }
    if !TYPES.contains(&kind.as_str()) {
        return Err(format!("@param {name} has type \"{kind}\". Use one of: {}", TYPES.join(", ")));
    }
    let (required, about) = match rest.strip_prefix("required") {
        Some(after) => (true, after.trim().to_string()),
        None => (false, rest.to_string()),
    };
    Ok(Param { name, kind, required, about })
}

/// The tools the code names, as `app/tool`, e.g. `tools.github.search_repositories` → `github/search_repositories`.
/// Only these may be called when the tool runs.
fn tools_named(source: &str) -> Vec<String> {
    let word = |s: &str| s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$').collect::<String>();
    let mut found: Vec<String> = source
        .match_indices("tools.")
        .filter(|(at, _)| !source[..*at].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .filter_map(|(at, _)| {
            let rest = &source[at + "tools.".len()..];
            let app = word(rest);
            let tool = word(rest[app.len()..].strip_prefix('.')?);
            (!app.is_empty() && !tool.is_empty()).then(|| format!("{app}/{tool}"))
        })
        .collect();
    found.sort();
    found.dedup();
    found
}

/// The tool's schema, in the same shape a server gives, so --help, the README and search all work.
fn schema_of(name: &str, header: &Header, uses: &[String]) -> Value {
    let properties: Map<String, Value> = header
        .params
        .iter()
        .map(|p| {
            let kind = if p.kind == "json" { "object" } else { p.kind.as_str() };
            (p.name.clone(), json!({ "type": kind, "description": p.about }))
        })
        .collect();
    let required: Vec<&str> = header.params.iter().filter(|p| p.required).map(|p| p.name.as_str()).collect();
    json!({
        "name": name,
        "description": header.description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
        "x-dogu-uses": uses,
    })
}

/// Save `source` as the tool `forged/<name>`. Returns a line saying how to call it.
pub fn save(root: &Path, name: &str, source: &str, replace: bool) -> Result<String, String> {
    check_name(name)?;
    if crate::sync::saved_state(root).is_none() {
        return Err(format!("{} is not a dogu folder. Run `dogu sync` first.", root.display()));
    }
    let header = parse_header(source)?;
    let uses = tools_named(source);
    if let Some(inner) = uses.iter().find(|u| u.starts_with(&format!("{FORGED}/"))) {
        return Err(format!("a forged tool cannot call another forged tool yet ({inner}). Copy its code in instead."));
    }
    let dir = root.join(FORGED);
    let program = dir.join(format!("{name}.ts"));
    if program.exists() && !replace {
        return Err(format!("{FORGED}/{name} exists. Pass --force to replace it."));
    }

    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let schema = schema_of(name, &header, &uses);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let schemas = schema_path(root, FORGED, name);
    fs::create_dir_all(schemas.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&program, source).map_err(|e| e.to_string())?;
    fs::write(&schemas, serde_json::to_string_pretty(&schema).unwrap()).map_err(|e| e.to_string())?;
    let stub = dir.join(name);
    fs::write(&stub, tool_stub(&me, root, FORGED, name)).map_err(|e| e.to_string())?;
    make_executable(&stub)?;
    fs::write(dir.join("index.d.ts"), types_file(FORGED, &saved_schemas(root, FORGED))).map_err(|e| e.to_string())?;
    write_root_readme(root)?;

    let calls = if uses.is_empty() { "no tools".to_string() } else { uses.join(", ") };
    Ok(format!(
        "Saved {FORGED}/{name}. It may call: {calls}.\nRun it as `{FORGED}/{name}{}` or `tools.{FORGED}.{name}({{...}})`.",
        crate::call::flags_of(&schema)
    ))
}

/// Run the forged tool with these arguments. Returns what it printed.
pub fn run(root: &Path, name: &str, args: &Value, pool: &mut Pool) -> Result<String, String> {
    let source = fs::read_to_string(root.join(FORGED).join(format!("{name}.ts")))
        .map_err(|_| format!("there is no {FORGED}/{name}"))?;
    let schema: Value = fs::read_to_string(schema_path(root, FORGED, name))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or_else(|| format!("{FORGED}/{name} has no saved schema. Save it again with ./forge --force."))?;
    let uses: Vec<String> = schema["x-dogu-uses"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default();

    let mut opts = Options::new(root.to_path_buf());
    // An empty allow list means "everything", so a tool that names no tools gets a pattern nothing matches.
    opts.allow = if uses.is_empty() { vec!["\u{0}".into()] } else { uses };
    let program = format!("const args = {args};\n{source}");
    let mut printed = String::new();
    let outcome = run_program(&opts, &program, pool, true, &mut |line| {
        printed.push_str(line);
        printed.push('\n');
    })?;
    match outcome.code {
        0 => Ok(printed),
        _ => Err(format!("{printed}{}", outcome.stderr).trim().to_string()),
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_tools_the_code_names() {
        let code = "const a = await tools.github.search_repositories({});\nawait tools.deepwiki.read_wiki_structure({});\nmytools.x.y; tools.github.search_repositories";

        assert_eq!(tools_named(code), vec!["deepwiki/read_wiki_structure", "github/search_repositories"]);
    }

    #[test]
    fn a_dynamic_call_names_no_tool() {
        assert!(tools_named("const t = 'count'; await tools.fx[t]({});").is_empty());
    }

    #[test]
    fn reads_the_header() {
        let h = parse_header("// @description Does a thing.\n// @param org string required The org\n// @param top number How many\n").unwrap();

        assert_eq!(h.description, "Does a thing.");
        assert_eq!(h.params.len(), 2);
        assert!(h.params[0].required && h.params[0].about == "The org");
        assert!(!h.params[1].required && h.params[1].kind == "number");
    }

    #[test]
    fn rejects_a_bad_param_type() {
        let e = parse_header("// @description x\n// @param org text required").err().unwrap();

        assert!(e.contains("string, number"), "{e}");
    }

    #[test]
    fn names_are_lowercase_words() {
        assert!(check_name("repo_activity").is_ok());
        assert!(check_name("Repo").is_err());
        assert!(check_name("a b").is_err());
        assert!(check_name("../x").is_err());
    }
}
