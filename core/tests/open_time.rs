//! The spec's bound: ten thousand notes open in under two seconds the second
//! time. Run: `cargo test --release -p engram-notes-core --test open_time -- --ignored --nocapture`

use engram_core::index::Index;
use engram_core::vault::Vault;
use std::time::{Duration, Instant};

#[test]
#[ignore = "benchmark, run in release"]
fn second_open_of_ten_thousand_notes_is_under_two_seconds() {
    let vault_dir = tempfile::tempdir().unwrap();
    let filler = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(20);
    for i in 0..10_000 {
        let dir = vault_dir.path().join(format!("f{}", i % 50));
        std::fs::create_dir_all(&dir).unwrap();
        let next = (i + 1) % 10_000;
        let far = (i * 7) % 10_000;
        let text = format!(
            "---\ntags: [t{}]\nn: {i}\n---\n# Note {i}\n\nSee [[Note {next}]] and [[Note {far}#Note {far}]] #topic/{}\n\n{filler}\n",
            i % 20,
            i % 30
        );
        std::fs::write(dir.join(format!("Note {i}.md")), text).unwrap();
    }
    let data_dir = tempfile::tempdir().unwrap();
    let db = data_dir.path().join("index.db");
    let open = || {
        let start = Instant::now();
        let vault = Vault::open(vault_dir.path()).unwrap();
        let (mut ix, _) = Index::open_or_recreate(&db).unwrap();
        let stats = ix.rebuild(&vault).unwrap();
        (start.elapsed(), stats)
    };
    let (first, s1) = open();
    let (second, s2) = open();
    println!(
        "first open {first:?} ({} added), second open {second:?} ({} unchanged)",
        s1.added, s2.unchanged
    );
    assert_eq!(s2.unchanged, 10_000);
    assert!(
        second < Duration::from_secs(2),
        "second open took {second:?}"
    );

    // A save re-indexes one file and re-resolves links; it must stay quick.
    let vault = Vault::open(vault_dir.path()).unwrap();
    let (mut ix, _) = Index::open_or_recreate(&db).unwrap();
    std::fs::write(vault_dir.path().join("f0/Note 0.md"), "changed [[Note 1]]").unwrap();
    let start = Instant::now();
    ix.update_file(&vault, "f0/Note 0.md").unwrap();
    let save = start.elapsed();
    println!("one save {save:?}");
    assert!(save < Duration::from_millis(500), "one save took {save:?}");
}
