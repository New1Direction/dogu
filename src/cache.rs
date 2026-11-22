//! A short memory of answers from read-only tools. An agent often calls a tool once to see what it
//! returns, then again with the same arguments to take a field out (22% of code-mode runs in the bench);
//! the second call is answered from here instead of going back to the server.
//!
//! Only tools the server marks read-only (`readOnlyHint`), only answers that are not errors, and only for
//! DOGU_CACHE_SECS seconds (30 by default; 0 turns it off). It lives in one process: `dogu serve`, a
//! `dogu run`, or the folder's background dogu.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const DEFAULT_SECS: u64 = 30;
/// Past this many answers, old ones are dropped first.
const MAX_ANSWERS: usize = 256;

static ANSWERS: Mutex<Option<HashMap<String, (Instant, Value)>>> = Mutex::new(None);

fn ttl() -> Duration {
    let secs = std::env::var("DOGU_CACHE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_SECS);
    Duration::from_secs(secs)
}

/// May this tool's answers be kept? Only when its server says it is read-only.
pub fn is_read_only(saved_tool: &Value) -> bool {
    saved_tool["annotations"]["readOnlyHint"] == true
}

/// The same tool with the same arguments, whatever order the arguments were written in. (This crate
/// keeps object keys in the order given, so they are sorted here.)
pub fn key(server: &str, tool: &str, arguments: &Value) -> String {
    format!("{server}\u{0}{tool}\u{0}{}", sorted(arguments))
}

fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            Value::Object(keys.into_iter().map(|k| (k.clone(), sorted(&map[k]))).collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

pub fn get(key: &str) -> Option<Value> {
    get_at(key, Instant::now(), ttl())
}

pub fn put(key: &str, answer: &Value) {
    put_at(key, answer, Instant::now(), ttl());
}

fn get_at(key: &str, now: Instant, ttl: Duration) -> Option<Value> {
    if ttl.is_zero() {
        return None;
    }
    let answers = ANSWERS.lock().ok()?;
    let (at, answer) = answers.as_ref()?.get(key)?;
    (now.saturating_duration_since(*at) < ttl).then(|| answer.clone())
}

fn put_at(key: &str, answer: &Value, now: Instant, ttl: Duration) {
    if ttl.is_zero() || answer["isError"] == true {
        return;
    }
    let Ok(mut answers) = ANSWERS.lock() else { return };
    let answers = answers.get_or_insert_with(HashMap::new);
    answers.retain(|_, (at, _)| now.saturating_duration_since(*at) < ttl);
    if answers.len() >= MAX_ANSWERS {
        if let Some(oldest) = answers.iter().min_by_key(|(_, (at, _))| *at).map(|(k, _)| k.clone()) {
            answers.remove(&oldest);
        }
    }
    answers.insert(key.to_string(), (now, answer.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_answer_is_kept_for_its_time_and_then_forgotten() {
        let (k, t0, ttl) = (key("s", "kept", &json!({ "a": 1 })), Instant::now(), Duration::from_secs(30));
        put_at(&k, &json!({ "content": [] }), t0, ttl);

        assert!(get_at(&k, t0 + Duration::from_secs(29), ttl).is_some());
        assert!(get_at(&k, t0 + Duration::from_secs(31), ttl).is_none());
        assert!(get_at(&k, t0, Duration::ZERO).is_none(), "a time of 0 turns it off");
    }

    #[test]
    fn errors_are_not_kept_and_argument_order_does_not_matter() {
        let t0 = Instant::now();
        let ttl = Duration::from_secs(30);
        put_at(&key("s", "err", &json!({})), &json!({ "isError": true }), t0, ttl);

        assert!(get_at(&key("s", "err", &json!({})), t0, ttl).is_none());
        let a: Value = serde_json::from_str(r#"{"x":1,"y":2}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"y":2,"x":1}"#).unwrap();
        assert_eq!(key("s", "t", &a), key("s", "t", &b));
    }

    #[test]
    fn only_tools_marked_read_only_are_kept() {
        assert!(is_read_only(&json!({ "annotations": { "readOnlyHint": true } })));
        assert!(!is_read_only(&json!({ "annotations": { "readOnlyHint": false } })));
        assert!(!is_read_only(&json!({})));
    }
}
