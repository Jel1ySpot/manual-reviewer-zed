use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use manual_reviewer_core::entry::{new_id, now_iso, CommentEntry, Position, PositionRange};
use manual_reviewer_core::{git, Store};

use crate::term_input::{print_selection_preview, read_multiline_comment};

const ENV_TEXT: &str = "MREVIEW_ZED_TEXT";
const ENV_FILE: &str = "MREVIEW_ZED_FILE";
const ENV_ROW: &str = "MREVIEW_ZED_ROW";
const ENV_COLUMN: &str = "MREVIEW_ZED_COLUMN";
const ENV_LANGUAGE: &str = "MREVIEW_ZED_LANGUAGE";
// MREVIEW_ZED_WORKTREE_ROOT is read in workspace::resolve, not here.

#[derive(Debug)]
enum Mode {
    /// User selected a code region.
    Snippet {
        file_abs: PathBuf,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
        snippet_text: String,
        language: String,
    },
    /// User selected the whole file (or used cmd-A then triggered).
    File {
        file_abs: PathBuf,
        language: String,
    },
    /// No selection; treated as a project-level note.
    Project,
}

pub fn run(workspace: &Path, message: Option<String>) -> Result<()> {
    let mode = detect_mode()?;
    show_preview(&mode, workspace)?;

    let comment = match message {
        Some(m) if !m.trim().is_empty() => m.trim().to_string(),
        _ => read_multiline_comment("Write your comment:")?,
    };
    if comment.is_empty() {
        return Err(anyhow!("Empty comment — entry not added."));
    }

    let mut store = Store::open(workspace)
        .with_context(|| format!("open store at {}", workspace.display()))?;
    let entry = build_entry(&mode, comment, workspace);
    let new_count = store.count() + 1;
    store.add(entry)?;
    println!(
        "Added entry #{}. Total: {} entries.",
        new_count,
        store.count()
    );
    Ok(())
}

fn detect_mode() -> Result<Mode> {
    let text = env::var(ENV_TEXT).unwrap_or_default();
    let file_var = env::var(ENV_FILE).unwrap_or_default();

    if text.is_empty() {
        // No selection → project-level (current file is irrelevant for this case).
        return Ok(Mode::Project);
    }

    if file_var.is_empty() {
        return Err(anyhow!(
            "${} is not set, but selection text is — is this command being launched from a Zed task?",
            ENV_FILE
        ));
    }
    let file_abs = PathBuf::from(&file_var);
    let language = env::var(ENV_LANGUAGE)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let start_line = parse_pos_var(ENV_ROW)?;
    let start_col = parse_pos_var(ENV_COLUMN)?;
    let (end_line, end_col) = derive_end_position(start_line, start_col, &text);

    if covers_whole_file(&file_abs, &text, start_line, start_col, end_line, end_col) {
        return Ok(Mode::File {
            file_abs,
            language,
        });
    }

    Ok(Mode::Snippet {
        file_abs,
        start_line,
        start_col,
        end_line,
        end_col,
        snippet_text: text,
        language,
    })
}

fn show_preview(mode: &Mode, workspace: &Path) -> Result<()> {
    match mode {
        Mode::Snippet {
            file_abs,
            start_line,
            start_col,
            end_line,
            end_col,
            snippet_text,
            language,
        } => {
            let rel = relative_to(workspace, file_abs);
            let label = format!("{}:{}-{}:{}", start_line, start_col, end_line, end_col);
            print_selection_preview(&rel, &label, language, snippet_text, *start_col)?;
        }
        Mode::File { file_abs, .. } => {
            let rel = relative_to(workspace, file_abs);
            println!("─── whole file: {} ─────────────────", rel);
        }
        Mode::Project => {
            println!("─── project-level note ─────────────────");
        }
    }
    Ok(())
}

fn build_entry(mode: &Mode, comment: String, workspace: &Path) -> CommentEntry {
    let now = now_iso();
    let git = git::capture(workspace);
    match mode {
        Mode::Snippet {
            file_abs,
            start_line,
            start_col,
            end_line,
            end_col,
            snippet_text,
            language,
        } => CommentEntry {
            id: new_id(),
            file: Some(relative_to(workspace, file_abs)),
            language: language.clone(),
            range: Some(PositionRange {
                start: Position {
                    line: *start_line,
                    column: *start_col,
                },
                end: Position {
                    line: *end_line,
                    column: *end_col,
                },
            }),
            snippet: snippet_text.clone(),
            comment,
            created_at: now,
            git,
        },
        Mode::File { file_abs, language } => CommentEntry {
            id: new_id(),
            file: Some(relative_to(workspace, file_abs)),
            language: language.clone(),
            range: None,
            snippet: String::new(),
            comment,
            created_at: now,
            git,
        },
        Mode::Project => CommentEntry {
            id: new_id(),
            file: None,
            language: String::new(),
            range: None,
            snippet: String::new(),
            comment,
            created_at: now,
            git,
        },
    }
}

fn parse_pos_var(name: &str) -> Result<u32> {
    let raw = env::var(name).map_err(|_| {
        anyhow!(
            "${} is not set — is this command being launched from a Zed task?",
            name
        )
    })?;
    raw.trim().parse::<u32>().map_err(|e| {
        anyhow!(
            "${} = {:?} is not a valid 1-based number ({})",
            name,
            raw,
            e
        )
    })
}

/// Compute (endLine, endCol) for the selection given start position and selected text.
/// All positions are 1-based.
pub fn derive_end_position(start_line: u32, start_col: u32, text: &str) -> (u32, u32) {
    if text.is_empty() {
        return (start_line, start_col);
    }
    let newlines = text.matches('\n').count() as u32;
    if newlines == 0 {
        let cols = text.chars().count() as u32;
        return (start_line, start_col + cols);
    }
    let end_line = start_line + newlines;
    let last_segment = text.rsplit('\n').next().unwrap_or("");
    let end_col = last_segment.chars().count() as u32 + 1;
    (end_line, end_col)
}

/// True iff the selection range exactly matches the file's full extent.
fn covers_whole_file(
    file_abs: &Path,
    text: &str,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
) -> bool {
    if start_line != 1 || start_col != 1 {
        return false;
    }
    let bytes = match fs::read(file_abs) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let file_text = String::from_utf8_lossy(&bytes);
    let file_no_trailing = file_text.strip_suffix('\n').unwrap_or(&file_text);
    if file_no_trailing.is_empty() {
        return false;
    }
    let text_no_trailing = text.strip_suffix('\n').unwrap_or(text);
    if file_no_trailing != text_no_trailing {
        return false;
    }
    let last_line_chars = file_no_trailing
        .rsplit('\n')
        .next()
        .map(|l| l.chars().count() as u32)
        .unwrap_or(0);
    let expected_end_line = file_no_trailing.matches('\n').count() as u32 + 1;
    end_line == expected_end_line && end_col == last_line_chars + 1
}

fn relative_to(workspace: &Path, file: &Path) -> String {
    let abs_workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let abs_file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    match abs_file.strip_prefix(&abs_workspace) {
        Ok(rel) => rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => abs_file.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_end_single_line_extends_column() {
        let (l, c) = derive_end_position(3, 5, "hello");
        assert_eq!((l, c), (3, 10));
    }

    #[test]
    fn derive_end_multi_line_uses_last_line_length() {
        let (l, c) = derive_end_position(10, 1, "ab\ncde\nfgh");
        assert_eq!((l, c), (12, 4));
    }

    #[test]
    fn derive_end_trailing_newline_lands_on_next_empty_line() {
        let (l, c) = derive_end_position(5, 1, "ab\n");
        assert_eq!((l, c), (6, 1));
    }

    #[test]
    fn derive_end_unicode_counts_codepoints() {
        let (l, c) = derive_end_position(1, 1, "ä🦀");
        assert_eq!((l, c), (1, 3));
    }

    #[test]
    fn derive_end_empty_text_no_op() {
        let (l, c) = derive_end_position(7, 9, "");
        assert_eq!((l, c), (7, 9));
    }

    #[test]
    fn relative_to_strips_workspace_prefix() {
        let tmp = std::env::temp_dir();
        let ws = tmp.join("rel-ws-test");
        let nested = ws.join("a/b/c.rs");
        std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
        std::fs::write(&nested, "x").unwrap();
        let rel = relative_to(&ws, &nested);
        assert_eq!(rel, "a/b/c.rs");
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn covers_whole_file_detects_full_match() {
        let tmp = std::env::temp_dir().join("mreview-whole-file");
        std::fs::create_dir_all(&tmp).unwrap();
        let f = tmp.join("a.rs");
        std::fs::write(&f, "abc\ndef\n").unwrap();
        assert!(covers_whole_file(&f, "abc\ndef", 1, 1, 2, 4));
        assert!(!covers_whole_file(&f, "abc\ndef", 1, 2, 2, 4));
        assert!(!covers_whole_file(&f, "abc\ndef", 1, 1, 2, 5));
        assert!(!covers_whole_file(&f, "abc\nde", 1, 1, 2, 3));
        std::fs::remove_dir_all(&tmp).ok();
    }
}
