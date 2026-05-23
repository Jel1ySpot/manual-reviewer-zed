use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::entry::{now_iso, CommentEntry, Session, SCHEMA_VERSION};

pub const SESSION_DIR: &str = ".mreview";
pub const SESSION_FILE: &str = "session.json";
pub const ARCHIVE_DIR: &str = "archive";

pub struct Store {
    workspace_root: PathBuf,
    session: Session,
}

impl Store {
    pub fn open(workspace_root: impl AsRef<Path>) -> Result<Self> {
        let root = workspace_root.as_ref().to_path_buf();
        let session = read_session(&root)?;
        Ok(Self {
            workspace_root: root,
            session,
        })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn entries(&self) -> &[CommentEntry] {
        &self.session.entries
    }

    pub fn count(&self) -> usize {
        self.session.entries.len()
    }

    pub fn get(&self, id: &str) -> Option<&CommentEntry> {
        self.session.entries.iter().find(|e| e.id == id)
    }

    pub fn add(&mut self, entry: CommentEntry) -> Result<()> {
        self.session.entries.push(entry);
        self.persist()
    }

    pub fn remove(&mut self, id: &str) -> Result<bool> {
        let before = self.session.entries.len();
        self.session.entries.retain(|e| e.id != id);
        let removed = self.session.entries.len() < before;
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    pub fn clear(&mut self, archive: bool) -> Result<()> {
        if archive && !self.session.entries.is_empty() {
            self.archive_current()?;
        }
        self.session = Session::empty(self.workspace_root.to_string_lossy());
        self.persist()
    }

    pub fn archive_prompt_file(&self, prompt_path: &Path) -> Result<Option<PathBuf>> {
        if !prompt_path.exists() {
            return Ok(None);
        }
        let stamp = now_iso().replace([':', '.'], "-");
        let archive_dir = self.workspace_root.join(SESSION_DIR).join(ARCHIVE_DIR);
        fs::create_dir_all(&archive_dir)
            .with_context(|| format!("create archive dir {}", archive_dir.display()))?;
        let archive_path = archive_dir.join(format!("PROMPT-{}.md", stamp));
        fs::copy(prompt_path, &archive_path).with_context(|| {
            format!(
                "archive {} → {}",
                prompt_path.display(),
                archive_path.display()
            )
        })?;
        Ok(Some(archive_path))
    }

    fn archive_current(&self) -> Result<()> {
        let stamp = now_iso().replace([':', '.'], "-");
        let archive_dir = self.workspace_root.join(SESSION_DIR).join(ARCHIVE_DIR);
        fs::create_dir_all(&archive_dir)?;
        let archive_path = archive_dir.join(format!("session-{}.json", stamp));
        let bytes = serde_json::to_vec_pretty(&self.session)?;
        fs::write(&archive_path, bytes)?;
        Ok(())
    }

    fn persist(&self) -> Result<()> {
        let dir = self.workspace_root.join(SESSION_DIR);
        fs::create_dir_all(&dir)?;
        let final_path = dir.join(SESSION_FILE);
        let tmp_path = dir.join(format!("{}.tmp", SESSION_FILE));
        let bytes = serde_json::to_vec_pretty(&self.session)?;
        {
            let mut f = fs::File::create(&tmp_path)?;
            f.write_all(&bytes)?;
            f.sync_all().ok();
        }
        // Atomic on POSIX; on filesystems where rename is unavailable, fall back.
        if let Err(rename_err) = fs::rename(&tmp_path, &final_path) {
            fs::write(&final_path, &bytes)
                .with_context(|| format!("write {}", final_path.display()))?;
            let _ = fs::remove_file(&tmp_path);
            // Surface rename hint only if direct write also failed; otherwise quiet.
            drop(rename_err);
        }
        Ok(())
    }

    pub fn session_path(&self) -> PathBuf {
        self.workspace_root.join(SESSION_DIR).join(SESSION_FILE)
    }
}

fn read_session(workspace_root: &Path) -> Result<Session> {
    let session_path = workspace_root.join(SESSION_DIR).join(SESSION_FILE);
    if !session_path.exists() {
        return Ok(Session::empty(workspace_root.to_string_lossy()));
    }
    let bytes =
        fs::read(&session_path).with_context(|| format!("read {}", session_path.display()))?;
    let parsed: Session = match serde_json::from_slice(&bytes) {
        Ok(s) => s,
        Err(_) => {
            // Corrupted or future-version file → start fresh, don't blow up.
            return Ok(Session::empty(workspace_root.to_string_lossy()));
        }
    };
    if parsed.schema_version != SCHEMA_VERSION {
        return Ok(Session::empty(workspace_root.to_string_lossy()));
    }
    Ok(Session {
        workspace_root: workspace_root.to_string_lossy().into_owned(),
        ..parsed
    })
}

/// Walk up from `start` looking for a `.git` directory; if not found, return `start` itself.
pub fn find_workspace_root(start: &Path) -> PathBuf {
    let mut cur = start;
    loop {
        if cur.join(".git").exists() {
            return cur.to_path_buf();
        }
        match cur.parent() {
            Some(p) => cur = p,
            None => return start.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{CommentEntry, GitInfo, Position, PositionRange};
    use tempfile::TempDir;

    fn sample_entry(id: &str) -> CommentEntry {
        CommentEntry {
            id: id.into(),
            file: Some("src/a.rs".into()),
            language: "rust".into(),
            range: Some(PositionRange {
                start: Position { line: 1, column: 1 },
                end: Position { line: 3, column: 8 },
            }),
            snippet: "let x = 1;".into(),
            comment: "rename".into(),
            created_at: "2026-05-08T00:00:00.000Z".into(),
            git: Some(GitInfo {
                sha: "abc1234".into(),
                branch: "main".into(),
                dirty: false,
            }),
        }
    }

    #[test]
    fn add_persists_then_reload_returns_same() {
        let tmp = TempDir::new().unwrap();
        let mut store = Store::open(tmp.path()).unwrap();
        store.add(sample_entry("e1")).unwrap();
        store.add(sample_entry("e2")).unwrap();
        let reload = Store::open(tmp.path()).unwrap();
        assert_eq!(reload.count(), 2);
        assert_eq!(reload.entries()[0].id, "e1");
    }

    #[test]
    fn remove_persists() {
        let tmp = TempDir::new().unwrap();
        let mut store = Store::open(tmp.path()).unwrap();
        store.add(sample_entry("a")).unwrap();
        store.add(sample_entry("b")).unwrap();
        assert!(store.remove("a").unwrap());
        let reload = Store::open(tmp.path()).unwrap();
        assert_eq!(reload.count(), 1);
        assert_eq!(reload.entries()[0].id, "b");
    }

    #[test]
    fn clear_with_archive_writes_archive_and_empties_session() {
        let tmp = TempDir::new().unwrap();
        let mut store = Store::open(tmp.path()).unwrap();
        store.add(sample_entry("e1")).unwrap();
        store.clear(true).unwrap();
        assert_eq!(store.count(), 0);
        let archive_dir = tmp.path().join(SESSION_DIR).join(ARCHIVE_DIR);
        assert!(archive_dir.exists());
        let entries: Vec<_> = fs::read_dir(&archive_dir).unwrap().collect();
        assert!(!entries.is_empty(), "expected archive file");
    }

    #[test]
    fn missing_session_yields_empty_store() {
        let tmp = TempDir::new().unwrap();
        let store = Store::open(tmp.path()).unwrap();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn corrupted_session_falls_back_to_empty() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(SESSION_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(SESSION_FILE), b"this is not json").unwrap();
        let store = Store::open(tmp.path()).unwrap();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn find_workspace_root_walks_up_to_git() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let nested = tmp.path().join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        let found = find_workspace_root(&nested);
        assert_eq!(found, tmp.path());
    }
}
