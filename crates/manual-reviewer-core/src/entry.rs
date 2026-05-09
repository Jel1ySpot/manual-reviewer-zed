use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PositionRange {
    pub start: Position,
    pub end: Position,
}

impl PositionRange {
    pub fn format(&self) -> String {
        format!(
            "{}:{}-{}:{}",
            self.start.line, self.start.column, self.end.line, self.end.column
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitInfo {
    pub sha: String,
    pub branch: String,
    pub dirty: bool,
}

/// Three kinds of entries:
/// - `Snippet` — code selection, has `file` + `range` + `snippet`
/// - `File` — whole-file edit, has `file` only
/// - `Project` — repo-wide note, has neither
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Snippet,
    File,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommentEntry {
    pub id: String,
    /// Workspace-relative path. `None` for project-level entries.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub file: Option<String>,
    #[serde(default)]
    pub language: String,
    /// `None` for whole-file and project-level entries.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub range: Option<PositionRange>,
    /// Empty for whole-file and project-level entries.
    #[serde(default)]
    pub snippet: String,
    pub comment: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub git: Option<GitInfo>,
}

impl CommentEntry {
    pub fn kind(&self) -> EntryKind {
        match (self.file.as_ref(), self.range.as_ref()) {
            (Some(_), Some(_)) => EntryKind::Snippet,
            (Some(_), None) => EntryKind::File,
            (None, _) => EntryKind::Project,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub schema_version: u32,
    pub workspace_root: String,
    pub created_at: String,
    pub entries: Vec<CommentEntry>,
}

impl Session {
    pub fn empty(workspace_root: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            workspace_root: workspace_root.into(),
            created_at: now_iso(),
            entries: Vec::new(),
        }
    }
}

pub fn now_iso() -> String {
    // Match JS `new Date().toISOString()` — ISO 8601 with millisecond precision and trailing Z.
    let now = chrono::Utc::now();
    now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_round_trips_through_json() {
        let s = Session {
            schema_version: 1,
            workspace_root: "/tmp/repo".into(),
            created_at: "2026-05-08T00:00:00.000Z".into(),
            entries: vec![CommentEntry {
                id: "id-1".into(),
                file: Some("src/foo.rs".into()),
                language: "rust".into(),
                range: Some(PositionRange {
                    start: Position { line: 10, column: 1 },
                    end: Position { line: 20, column: 12 },
                }),
                snippet: "let x = 1;".into(),
                comment: "rename".into(),
                created_at: "2026-05-08T00:00:00.000Z".into(),
                git: Some(GitInfo {
                    sha: "abc".into(),
                    branch: "main".into(),
                    dirty: true,
                }),
            }],
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            json.contains("\"schemaVersion\":1"),
            "expected camelCase schemaVersion, got: {}",
            json
        );
        assert!(json.contains("\"workspaceRoot\""));
        assert!(json.contains("\"createdAt\""));
        let back: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn entry_kind_classifies_three_modes() {
        let mut e = CommentEntry {
            id: "x".into(),
            file: Some("a.rs".into()),
            language: "rust".into(),
            range: Some(PositionRange {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 2 },
            }),
            snippet: "a".into(),
            comment: "c".into(),
            created_at: "2026-05-08T00:00:00.000Z".into(),
            git: None,
        };
        assert_eq!(e.kind(), EntryKind::Snippet);
        e.range = None;
        assert_eq!(e.kind(), EntryKind::File);
        e.file = None;
        assert_eq!(e.kind(), EntryKind::Project);
    }

    #[test]
    fn project_entry_omits_optional_fields_in_json() {
        let e = CommentEntry {
            id: "x".into(),
            file: None,
            language: String::new(),
            range: None,
            snippet: String::new(),
            comment: "project-wide note".into(),
            created_at: "2026-05-08T00:00:00.000Z".into(),
            git: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("\"file\""), "expected file omitted, got: {}", json);
        assert!(!json.contains("\"range\""), "expected range omitted, got: {}", json);
        assert!(!json.contains("\"git\""), "expected git omitted, got: {}", json);
    }

    #[test]
    fn position_range_format_matches_vscode() {
        let r = PositionRange {
            start: Position { line: 8, column: 0 },
            end: Position { line: 12, column: 23 },
        };
        assert_eq!(r.format(), "8:0-12:23");
    }

    #[test]
    fn entry_skips_git_field_when_none() {
        let e = CommentEntry {
            id: "x".into(),
            file: Some("a.ts".into()),
            language: "typescript".into(),
            range: Some(PositionRange {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 2 },
            }),
            snippet: "a".into(),
            comment: "c".into(),
            created_at: "2026-05-08T00:00:00.000Z".into(),
            git: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("\"git\""), "expected git omitted, got: {}", json);
    }

    #[test]
    fn now_iso_has_millis_and_z() {
        let s = now_iso();
        // Format: 2026-05-08T17:23:45.123Z
        assert!(s.ends_with('Z'), "expected trailing Z, got: {}", s);
        assert!(s.contains('.'), "expected millisecond fractional, got: {}", s);
    }
}
