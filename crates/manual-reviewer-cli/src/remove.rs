use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{anyhow, Result};
use manual_reviewer_core::Store;

pub fn run(workspace: &Path, spec: &str) -> Result<()> {
    let mut store = Store::open(workspace)?;
    let total = store.count();
    let target_ids = resolve_targets(spec, &store)?;
    if target_ids.is_empty() {
        println!("No entries matched.");
        return Ok(());
    }
    let mut removed = 0usize;
    for id in &target_ids {
        if store.remove(id)? {
            removed += 1;
        }
    }
    println!(
        "Removed {} entr{} ({} → {}).",
        removed,
        if removed == 1 { "y" } else { "ies" },
        total,
        store.count()
    );
    Ok(())
}

/// `spec` may be:
/// - a UUID (anything that doesn't contain `,` or `-` and isn't a number)
/// - a 1-based index: `3`
/// - a comma list of indices: `1,3,5`
/// - an inclusive range of indices: `2-4`
/// - any combination: `1,3-5,8`
fn resolve_targets(spec: &str, store: &Store) -> Result<Vec<String>> {
    let total = store.count();
    let trimmed = spec.trim();

    if !trimmed.contains(',') && !trimmed.contains('-') {
        if let Ok(idx) = trimmed.parse::<usize>() {
            return Ok(vec![one(idx, total, store)?]);
        }
        return Ok(vec![trimmed.to_string()]);
    }

    let mut indices: BTreeSet<usize> = BTreeSet::new();
    for piece in trimmed.split(',') {
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        if let Some((lo, hi)) = piece.split_once('-') {
            let lo: usize = lo
                .trim()
                .parse()
                .map_err(|_| anyhow!("range start {:?} is not a 1-based index", lo))?;
            let hi: usize = hi
                .trim()
                .parse()
                .map_err(|_| anyhow!("range end {:?} is not a 1-based index", hi))?;
            if lo == 0 || hi == 0 || lo > hi {
                return Err(anyhow!("invalid range {}-{}", lo, hi));
            }
            for i in lo..=hi {
                indices.insert(i);
            }
        } else {
            let i: usize = piece
                .parse()
                .map_err(|_| anyhow!("{:?} is not a 1-based index", piece))?;
            indices.insert(i);
        }
    }

    let mut ids = Vec::with_capacity(indices.len());
    for i in indices {
        ids.push(one(i, total, store)?);
    }
    Ok(ids)
}

fn one(idx: usize, total: usize, store: &Store) -> Result<String> {
    if idx == 0 || idx > total {
        return Err(anyhow!("index {} is out of range (1..={})", idx, total));
    }
    Ok(store.entries()[idx - 1].id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use manual_reviewer_core::entry::{CommentEntry, Position, PositionRange};
    use tempfile::TempDir;

    fn entry(id: &str) -> CommentEntry {
        CommentEntry {
            id: id.into(),
            file: Some("a.rs".into()),
            language: "rust".into(),
            range: Some(PositionRange {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 2 },
            }),
            snippet: "x".into(),
            comment: "c".into(),
            created_at: "2026-05-08T00:00:00.000Z".into(),
            git: None,
        }
    }

    fn store_with(n: usize) -> (TempDir, Store) {
        let tmp = TempDir::new().unwrap();
        let mut s = Store::open(tmp.path()).unwrap();
        for i in 1..=n {
            s.add(entry(&format!("id-{}", i))).unwrap();
        }
        (tmp, s)
    }

    #[test]
    fn parses_single_index() {
        let (_t, s) = store_with(3);
        let ids = resolve_targets("2", &s).unwrap();
        assert_eq!(ids, vec!["id-2"]);
    }

    #[test]
    fn parses_comma_list() {
        let (_t, s) = store_with(5);
        let ids = resolve_targets("1,3,5", &s).unwrap();
        assert_eq!(ids, vec!["id-1", "id-3", "id-5"]);
    }

    #[test]
    fn parses_range() {
        let (_t, s) = store_with(5);
        let ids = resolve_targets("2-4", &s).unwrap();
        assert_eq!(ids, vec!["id-2", "id-3", "id-4"]);
    }

    #[test]
    fn parses_combo_dedups() {
        let (_t, s) = store_with(6);
        let ids = resolve_targets("1,3-5,4,6", &s).unwrap();
        assert_eq!(ids, vec!["id-1", "id-3", "id-4", "id-5", "id-6"]);
    }

    #[test]
    fn out_of_range_errors() {
        let (_t, s) = store_with(2);
        assert!(resolve_targets("3", &s).is_err());
        assert!(resolve_targets("1-5", &s).is_err());
    }
}
