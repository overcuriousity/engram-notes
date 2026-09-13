//! Nodes and edges for the graph view.

use crate::Result;
use crate::index::Index;
use crate::vault::{FileEntry, normalize_rel};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Note,
    Attachment,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphNode {
    /// The vault path, or the link text for an unresolved target.
    pub id: String,
    pub title: String,
    pub kind: NodeKind,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn build(index: &Index, files: &[FileEntry]) -> Result<Graph> {
    let conn = index.conn();
    let mut tags: HashMap<String, Vec<String>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT path, tag FROM tags ORDER BY path, tag")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (path, tag) = row?;
        tags.entry(path).or_default().push(tag);
    }
    let mut nodes: Vec<GraphNode> = index
        .titles()?
        .into_iter()
        .map(|(path, title)| GraphNode {
            tags: tags.remove(&path).unwrap_or_default(),
            id: path,
            title,
            kind: NodeKind::Note,
        })
        .collect();

    // Attachments resolve like notes: exact path, then name, shortest path first.
    let mut attachments: Vec<&FileEntry> = files.iter().filter(|f| !f.is_markdown).collect();
    attachments.sort_by_key(|f| (f.path.len(), f.path.as_str()));
    let mut by_name: HashMap<String, &str> = HashMap::new();
    for f in &attachments {
        by_name.entry(f.path.to_lowercase()).or_insert(&f.path);
    }
    for f in &attachments {
        by_name
            .entry(name(&f.path).to_lowercase())
            .or_insert(&f.path);
    }

    let mut unresolved = Vec::new();
    let mut seen = HashSet::new();
    let mut edges = Vec::new();
    let mut pairs = HashSet::new();
    let mut stmt = conn
        .prepare("SELECT src_path, target_raw, target_path FROM links ORDER BY src_path, start")?;
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })? {
        let (source, raw, resolved) = row?;
        let target = match resolved {
            Some(t) => t,
            None => match by_name.get(&normalize_rel(&raw).to_lowercase()) {
                Some(p) => (*p).to_owned(),
                None => {
                    if seen.insert(raw.clone()) {
                        unresolved.push(GraphNode {
                            id: raw.clone(),
                            title: raw.clone(),
                            kind: NodeKind::Unresolved,
                            tags: vec![],
                        });
                    }
                    raw
                }
            },
        };
        if source != target && pairs.insert((source.clone(), target.clone())) {
            edges.push(GraphEdge { source, target });
        }
    }
    nodes.extend(unresolved);
    nodes.extend(files.iter().filter(|f| !f.is_markdown).map(|f| GraphNode {
        id: f.path.clone(),
        title: name(&f.path).to_owned(),
        kind: NodeKind::Attachment,
        tags: vec![],
    }));
    Ok(Graph { nodes, edges })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SemanticKind {
    /// A link the memory learned from co-retrieval.
    Assoc,
    /// Near passages by embedding.
    Similar,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SemanticEdge {
    pub source: String,
    pub target: String,
    pub weight: f32,
    pub kind: SemanticKind,
}

/// The edges the graph draws dashed: what the vault is about, not what was
/// linked by hand. `paths` narrows it to one neighbourhood; `None` is the vault.
///
/// Undirected, so a pair is stored with the smaller path first and drawn once.
pub fn semantic_edges(
    index: &Index,
    paths: Option<&[String]>,
    top_k: usize,
    cfg: &crate::config::MemoryConfig,
    search: &crate::config::SearchConfig,
    at: i64,
) -> Result<Vec<SemanticEdge>> {
    let all: Vec<String>;
    let subset: &[String] = match paths {
        Some(p) => p,
        None => {
            all = index.titles()?.into_iter().map(|(p, _)| p).collect();
            &all
        }
    };
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut out = Vec::new();
    for row in index.assoc_from(subset, at, cfg)? {
        let (a, b) = if row.via < row.other {
            (row.via.clone(), row.other.clone())
        } else {
            (row.other.clone(), row.via.clone())
        };
        if seen.insert((a.clone(), b.clone())) {
            out.push(SemanticEdge {
                source: a,
                target: b,
                weight: row.value as f32,
                kind: SemanticKind::Assoc,
            });
        }
    }
    for path in subset {
        for hit in index.similar_to(std::slice::from_ref(path), top_k, search.similarity_floor)? {
            let (a, b) = if *path < hit.path {
                (path.clone(), hit.path.clone())
            } else {
                (hit.path.clone(), path.clone())
            };
            if hit.similarity > 0.0 && seen.insert((a.clone(), b.clone())) {
                out.push(SemanticEdge {
                    source: a,
                    target: b,
                    weight: hit.similarity,
                    kind: SemanticKind::Similar,
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use std::fs;

    #[test]
    fn notes_attachments_unresolved_and_one_edge_per_pair() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("sub")).unwrap();
        fs::create_dir_all(d.path().join("img")).unwrap();
        fs::write(
            d.path().join("A.md"),
            "[[B]] [[B]] [[Ghost]] ![[pic.png]] #t",
        )
        .unwrap();
        fs::write(d.path().join("sub/B.md"), "[[A]] [[B]]").unwrap();
        fs::write(d.path().join("img/pic.png"), "p").unwrap();
        fs::write(d.path().join("other.pdf"), "p").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let g = build(&ix, &v.walk().unwrap()).unwrap();
        let nodes: Vec<_> = g
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.title.as_str(), n.kind))
            .collect();
        assert_eq!(
            nodes,
            vec![
                ("A.md", "A", NodeKind::Note),
                ("sub/B.md", "B", NodeKind::Note),
                ("Ghost", "Ghost", NodeKind::Unresolved),
                ("img/pic.png", "pic.png", NodeKind::Attachment),
                ("other.pdf", "other.pdf", NodeKind::Attachment),
            ]
        );
        assert_eq!(g.nodes[0].tags, vec!["t"]);
        let edges: Vec<_> = g
            .edges
            .iter()
            .map(|e| (e.source.as_str(), e.target.as_str()))
            .collect();
        assert_eq!(
            edges,
            vec![
                ("A.md", "sub/B.md"),
                ("A.md", "Ghost"),
                ("A.md", "img/pic.png"),
                ("sub/B.md", "A.md"),
            ]
        );
    }

    fn loose() -> crate::config::SearchConfig {
        crate::config::SearchConfig {
            similarity_floor: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn semantic_edges_carry_associations_and_near_passages() {
        use crate::config::MemoryConfig;
        use crate::embed::{Embedder, FakeEmbedder};
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("Rust.md"), "ownership rules keep memory safe").unwrap();
        std::fs::write(d.path().join("Borrow.md"), "ownership rules and borrowing").unwrap();
        std::fs::write(d.path().join("Coffee.md"), "kettle grind beans").unwrap();
        let v = crate::vault::Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let mut e = FakeEmbedder::new(64);
        ix.set_model_id(&e.id()).unwrap();
        let pending = ix.pending_vectors(1000).unwrap();
        let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
        let vecs = e.embed_documents(&texts).unwrap();
        let rows: Vec<(String, Vec<f32>)> = pending.into_iter().map(|p| p.hash).zip(vecs).collect();
        ix.put_vectors(&rows).unwrap();
        let cfg = MemoryConfig::default();
        ix.bump_assoc("Rust.md", "Coffee.md", 5.0, None, &cfg, 0)
            .unwrap();

        // Undirected: a pair is stored and drawn with the smaller path first.
        let all = semantic_edges(&ix, None, 1, &cfg, &loose(), 0).unwrap();
        assert!(all.iter().any(|e| e.kind == SemanticKind::Assoc
            && e.source == "Coffee.md"
            && e.target == "Rust.md"));
        let similar: Vec<_> = all
            .iter()
            .filter(|e| e.kind == SemanticKind::Similar)
            .collect();
        assert!(
            similar
                .iter()
                .any(|e| { (e.source.as_str(), e.target.as_str()) == ("Borrow.md", "Rust.md") })
        );
        let mut pairs: Vec<(String, String)> = similar
            .iter()
            .map(|e| (e.source.clone(), e.target.clone()))
            .collect();
        pairs.sort();
        pairs.dedup();
        assert_eq!(pairs.len(), similar.len());
        assert!(similar.iter().all(|e| e.weight > 0.0 && e.weight <= 1.0));
    }

    #[test]
    fn asking_for_one_note_gives_only_its_edges() {
        use crate::config::MemoryConfig;
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.md"), "a").unwrap();
        std::fs::write(d.path().join("B.md"), "b").unwrap();
        std::fs::write(d.path().join("C.md"), "c").unwrap();
        let v = crate::vault::Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let cfg = MemoryConfig::default();
        ix.bump_assoc("A.md", "B.md", 5.0, None, &cfg, 0).unwrap();
        ix.bump_assoc("B.md", "C.md", 5.0, None, &cfg, 0).unwrap();
        let only_a =
            semantic_edges(&ix, Some(&["A.md".to_string()]), 3, &cfg, &loose(), 0).unwrap();
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].target, "B.md");
    }
}
