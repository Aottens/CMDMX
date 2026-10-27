//! Fixture library: importers that turn foreign fixture files into [`crate::profile::Profile`]s
//! (ADR-006, `specs/modules/fixture-library.md`), the channel-list builder (LIB-11) and the
//! search [`Index`] (LIB-12..14). Pure: importers take the file's text (QLC+) or bytes (GDTF),
//! the index holds names.

pub mod channel_list;
pub mod gdtf;
pub mod qlcplus;

use std::collections::BTreeMap;

use serde::Serialize;

/// A mode of a library entry: its name and how many DMX channels it takes (LIB-19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntryMode {
    pub name: String,
    pub channels: u16,
}

/// A manufacturer and how many models it has (LIB-20).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Manufacturer {
    pub name: String,
    pub models: usize,
}

/// One fixture type in the library: what a search lists (LIB-14).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub manufacturer: String,
    pub model: String,
    pub modes: Vec<EntryMode>,
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
        self.matching(query).into_iter().take(limit).collect()
    }

    /// Manufacturers with at least one model matching `query` (LIB-14 words), with that count,
    /// sorted case-insensitively (LIB-20).
    pub fn manufacturers(&self, query: &str) -> Vec<Manufacturer> {
        let mut out: Vec<Manufacturer> = Vec::new();
        for e in self.matching(query) {
            match out.last_mut() {
                Some(m) if m.name == e.manufacturer => m.models += 1,
                _ => out.push(Manufacturer {
                    name: e.manufacturer.clone(),
                    models: 1,
                }),
            }
        }
        out
    }

    /// Matching entries sorted by manufacturer, then model, case-insensitively (LIB-21). With a
    /// `manufacturer` (exact name) all its models are returned; without, at most `limit`, and
    /// the flag says whether more matched.
    pub fn models(
        &self,
        manufacturer: Option<&str>,
        query: &str,
        limit: usize,
    ) -> (Vec<&Entry>, bool) {
        let mut found = self.matching(query);
        match manufacturer {
            Some(m) => {
                found.retain(|e| e.manufacturer == m);
                (found, false)
            }
            None => {
                let truncated = found.len() > limit;
                found.truncate(limit);
                (found, truncated)
            }
        }
    }

    /// LIB-14's word rule, sorted case-insensitively.
    fn matching(&self, query: &str) -> Vec<&Entry> {
        let query = query.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut found: Vec<&Entry> = self
            .0
            .values()
            .filter(|e| {
                let name = format!("{} {}", e.manufacturer, e.model).to_lowercase();
                words.iter().all(|w| name.contains(w))
            })
            .collect();
        // The exact name breaks ties so a manufacturer's models stay together (LIB-20).
        found.sort_by_cached_key(|e| {
            (
                e.manufacturer.to_lowercase(),
                e.manufacturer.clone(),
                e.model.to_lowercase(),
            )
        });
        found
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
