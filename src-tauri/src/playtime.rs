//! Time played per character, kept only on this computer (`playtime.json` in the data folder).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Sessions older than this are folded into the total only.
const KEEP_SECS: u64 = 90 * 86400;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    /// All time, in seconds.
    pub total: u64,
    /// Recent sessions as [start, end] unix seconds, so the window can count today and this week in local time.
    pub sessions: Vec<[u64; 2]>,
}

/// Keyed by `account id:character id`.
pub type Book = HashMap<String, Entry>;

pub fn path() -> PathBuf {
    crate::config::data_dir().join("playtime.json")
}

pub fn load(path: &Path) -> Book {
    std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn add(book: &mut Book, key: &str, start: u64, end: u64) {
    if end <= start {
        return;
    }
    let e = book.entry(key.to_string()).or_default();
    e.total += end - start;
    e.sessions.push([start, end]);
    e.sessions.retain(|s| s[1] + KEEP_SECS >= end);
}

pub fn record(key: &str, start: u64, end: u64) {
    let p = path();
    let mut book = load(&p);
    add(&mut book, key, start, end);
    if let Ok(json) = serde_json::to_vec(&book) {
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_trims_sessions() {
        let mut b = Book::new();
        add(&mut b, "a:1", 100, 160);
        add(&mut b, "a:1", 200, 200);
        assert_eq!(b["a:1"], Entry { total: 60, sessions: vec![[100, 160]] });
        add(&mut b, "a:1", KEEP_SECS + 1000, KEEP_SECS + 1100);
        assert_eq!(b["a:1"].total, 160);
        assert_eq!(b["a:1"].sessions, vec![[KEEP_SECS + 1000, KEEP_SECS + 1100]]);
    }
}
