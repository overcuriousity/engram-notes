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
}
