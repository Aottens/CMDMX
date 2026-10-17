//! Fixture library: importers that turn foreign fixture files into [`crate::profile::Profile`]s
//! (ADR-006, `specs/modules/fixture-library.md`), the channel-list builder (LIB-11) and the
//! search [`Index`] (LIB-12..14). Pure: importers take the file's text, the index holds names.

pub mod channel_list;
pub mod qlcplus;

use std::collections::BTreeMap;

use serde::Serialize;

/// One fixture type in the library: what a search lists (LIB-14).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub manufacturer: String,
    pub model: String,
    pub modes: Vec<String>,
    /// `"qlcplus"` (bundled, converted on pick) or `"user"` (a saved profile, LIB-12).
    pub source: String,
    /// Where the daemon finds the file; never sent to clients.
    #[serde(skip)]
    pub path: String,
}

/// Entries by manufacturer and model, so listing is already sorted (LIB-14).
#[derive(Debug, Clone, Default)]
pub struct Index(BTreeMap<(String, String), Entry>);

impl Index {
    /// A user entry replaces the same manufacturer/model; a QLC+ entry never replaces a user one (LIB-12).
    pub fn insert(&mut self, entry: Entry) {
        let key = (entry.manufacturer.clone(), entry.model.clone());
        if entry.source != "user" && self.0.get(&key).is_some_and(|e| e.source == "user") {
            return;
        }
        self.0.insert(key, entry);
    }

    pub fn get(&self, manufacturer: &str, model: &str) -> Option<&Entry> {
        self.0.get(&(manufacturer.to_string(), model.to_string()))
    }

    /// Entries in which every word of `query` occurs, case-insensitively, in
    /// "manufacturer model"; at most `limit`. An empty query lists everything (LIB-14).
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Entry> {
        let query = query.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        self.0
            .values()
            .filter(|e| {
                let name = format!("{} {}", e.manufacturer, e.model).to_lowercase();
                words.iter().all(|w| name.contains(w))
            })
            .take(limit)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
