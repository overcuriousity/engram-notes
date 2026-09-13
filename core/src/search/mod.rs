//! Full-text and semantic retrieval, fused, with a divider where relevance falls.

pub mod fuse;
pub mod passages;
pub mod vector;

/// One note in a result list, with the passage or snippet that found it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Hit {
    pub path: String,
    pub title: String,
    /// HTML-safe; the full-text branch marks its matches with `<mark>`.
    pub snippet: String,
    pub heading: Option<String>,
    /// 1-based line in the file, for jumping to the passage.
    pub line: u32,
    /// Cosine of the best passage; `None` when only full-text found it.
    pub similarity: Option<f32>,
    /// The fused rank score, for ordering only.
    pub score: f64,
    pub past_divider: bool,
    pub primed: bool,
}

/// A note that did not match but is associated with one that did.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Associated {
    pub path: String,
    pub title: String,
    /// The hit that recalled it.
    pub via: String,
    /// The query that bound them, when one did.
    pub cue: Option<String>,
    pub strength: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct SearchResults {
    pub hits: Vec<Hit>,
    pub associated: Vec<Associated>,
}
