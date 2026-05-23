use std::path::Path;

use crate::entry::{now_iso, CommentEntry, EntryKind, GitInfo, Session};

/// The default prompt template, embedded at compile time so the binary stays
/// self-contained. Must remain byte-identical with
/// `manual-reviewer-vscode/prompt.template.md` (CI enforces this with a `diff`).
pub const DEFAULT_TEMPLATE: &str = include_str!("../../../prompt.template.md");

/// If a snippet exceeds this many lines, truncate to `SNIPPET_HEAD_LINES`
/// followed by an elision marker. Aligned with the terminal preview cap.
pub const MAX_SNIPPET_LINES: usize = 40;
pub const SNIPPET_HEAD_LINES: usize = 30;

pub struct RenderArgs<'a> {
    pub session: &'a Session,
    pub template: &'a str,
    pub repo_name: Option<&'a str>,
    pub git: Option<&'a GitInfo>,
    /// If `None`, the current ISO 8601 timestamp is used.
    pub timestamp: Option<String>,
}

pub fn render(args: RenderArgs<'_>) -> String {
    let timestamp = args.timestamp.unwrap_or_else(now_iso);
    let repo = args.repo_name.map(|s| s.to_string()).unwrap_or_else(|| {
        Path::new(&args.session.workspace_root)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string()
    });
    let git_info = format_git(args.git);
    let count = args.session.entries.len().to_string();
    let entries_block = if args.session.entries.is_empty() {
        "_(no entries)_".to_string()
    } else {
        args.session
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| render_entry(e, i + 1))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    };

    args.template
        .replace("{{timestamp}}", &timestamp)
        .replace("{{repo_name}}", &repo)
        .replace("{{git_info}}", &git_info)
        .replace("{{count}}", &count)
        .replace("{{entries}}", &entries_block)
}

fn format_git(git: Option<&GitInfo>) -> String {
    let g = match git {
        Some(g) => g,
        None => return String::new(),
    };
    let mut parts: Vec<String> = Vec::new();
    if !g.branch.is_empty() {
        parts.push(format!("branch `{}`", g.branch));
    }
    if !g.sha.is_empty() {
        let short: String = g.sha.chars().take(7).collect();
        parts.push(format!("sha `{}`", short));
    }
    parts.push((if g.dirty { "dirty" } else { "clean" }).to_string());
    format!(" ({})", parts.join(", "))
}

fn render_entry(entry: &CommentEntry, index: usize) -> String {
    match entry.kind() {
        EntryKind::Snippet => render_snippet_entry(entry, index),
        EntryKind::File => render_file_entry(entry, index),
        EntryKind::Project => render_project_entry(entry, index),
    }
}

fn render_snippet_entry(entry: &CommentEntry, index: usize) -> String {
    let file = entry.file.as_deref().unwrap_or("");
    let range = entry.range.as_ref().expect("snippet entry must have range");
    let snippet = truncate_snippet(&entry.snippet);
    let snippet = indent_first_line(&snippet, range.start.column);
    let lines: Vec<String> = vec![
        format!("## [{}] {}<{}>", index, file, range.format()),
        String::new(),
        format!("```{}", entry.language),
        snippet,
        "```".to_string(),
        String::new(),
        entry.comment.clone(),
    ];
    lines.join("\n")
}

fn render_file_entry(entry: &CommentEntry, index: usize) -> String {
    let file = entry.file.as_deref().unwrap_or("");
    let lines: Vec<String> = vec![
        format!("## [{}] {}", index, file),
        String::new(),
        entry.comment.clone(),
    ];
    lines.join("\n")
}

fn render_project_entry(entry: &CommentEntry, index: usize) -> String {
    let lines: Vec<String> = vec![
        format!("## [{}]", index),
        String::new(),
        entry.comment.clone(),
    ];
    lines.join("\n")
}

fn truncate_snippet(snippet: &str) -> String {
    let lines: Vec<&str> = snippet.split('\n').collect();
    if lines.len() <= MAX_SNIPPET_LINES {
        return snippet.to_string();
    }
    let head: Vec<&str> = lines.iter().take(SNIPPET_HEAD_LINES).copied().collect();
    let elided = lines.len() - SNIPPET_HEAD_LINES;
    let mut result = head.join("\n");
    result.push('\n');
    result.push_str(&format!("[... {} more lines elided ...]", elided));
    result
}

fn indent_first_line(snippet: &str, start_column: u32) -> String {
    let pad = start_column.saturating_sub(1) as usize;
    if pad == 0 {
        return snippet.to_string();
    }
    // Single-line snippet: leave as-is. The leading indent only adds value
    // when there are subsequent lines to align against.
    match snippet.find('\n') {
        None => snippet.to_string(),
        Some(i) => {
            let indent: String = " ".repeat(pad);
            let mut out = String::with_capacity(snippet.len() + pad);
            out.push_str(&indent);
            out.push_str(&snippet[..i]);
            out.push_str(&snippet[i..]);
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{CommentEntry, GitInfo, Position, PositionRange, Session};

    const TEMPLATE: &str = "# Header — {{timestamp}}\n\
        Repository: {{repo_name}}{{git_info}}\n\
        Items: {{count}}\n\
        \n\
        {{entries}}\n";

    fn snippet_entry(id: &str, file: &str, lang: &str) -> CommentEntry {
        CommentEntry {
            id: id.into(),
            file: Some(file.into()),
            language: lang.into(),
            range: Some(PositionRange {
                start: Position { line: 1, column: 1 },
                end: Position { line: 3, column: 8 },
            }),
            snippet: "let x = 1;\nlet y = 2;".into(),
            comment: "rename x to count".into(),
            created_at: "2026-05-08T00:00:00.000Z".into(),
            git: None,
        }
    }

    fn session(entries: Vec<CommentEntry>) -> Session {
        Session {
            schema_version: 1,
            workspace_root: "/tmp/repo".into(),
            created_at: "2026-05-08T00:00:00.000Z".into(),
            entries,
        }
    }

    #[test]
    fn empty_entries_uses_placeholder_and_no_git_block() {
        let s = session(vec![]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("T".into()),
            repo_name: None,
            git: None,
        });
        assert!(out.contains("Items: 0"));
        assert!(out.contains("_(no entries)_"));
        // No-git: line should be exactly "Repository: repo" with no parenthetical.
        assert!(out.contains("Repository: repo\n"), "got:\n{}", out);
    }

    #[test]
    fn single_snippet_entry_byte_for_byte() {
        let s = session(vec![snippet_entry("e1", "src/a.ts", "typescript")]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("2026-05-08T00:00:00.000Z".into()),
            repo_name: None,
            git: None,
        });
        let expected = "# Header — 2026-05-08T00:00:00.000Z\n\
            Repository: repo\n\
            Items: 1\n\
            \n\
            ## [1] src/a.ts<1:1-3:8>\n\
            \n\
            ```typescript\n\
            let x = 1;\n\
            let y = 2;\n\
            ```\n\
            \n\
            rename x to count\n";
        assert_eq!(out, expected, "rendered:\n{}\nexpected:\n{}", out, expected);
    }

    #[test]
    fn multi_entries_separated_by_dash_block_no_trailing() {
        let s = session(vec![
            snippet_entry("a", "a.ts", "typescript"),
            snippet_entry("b", "b.ts", "typescript"),
        ]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("T".into()),
            repo_name: None,
            git: None,
        });
        // Entries separated by "\n\n---\n\n"
        assert!(
            out.contains("rename x to count\n\n---\n\n## [2] b.ts<"),
            "got:\n{}",
            out
        );
        // Output should NOT end with a `---` line — only the per-entry trailing
        // comment + a final newline.
        assert!(out.ends_with("rename x to count\n"), "got:\n{}", out);
        assert!(
            !out.contains("rename x to count\n\n---\n\n\n"),
            "no trailing ---"
        );
    }

    #[test]
    fn git_present_yields_inline_paren() {
        let g = GitInfo {
            sha: "abcdef0123456789".into(),
            branch: "main".into(),
            dirty: true,
        };
        let s = session(vec![snippet_entry("e", "a.rs", "rust")]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            git: Some(&g),
            timestamp: Some("T".into()),
            repo_name: None,
        });
        assert!(
            out.contains("Repository: repo (branch `main`, sha `abcdef0`, dirty)"),
            "got:\n{}",
            out
        );
    }

    #[test]
    fn empty_language_yields_bare_fence() {
        let s = session(vec![snippet_entry("e", "a.txt", "")]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("T".into()),
            repo_name: None,
            git: None,
        });
        assert!(out.contains("\n```\nlet x = 1;\n"), "got: {}", out);
    }

    #[test]
    fn first_line_indented_to_match_start_column() {
        let mut e = snippet_entry("e", "a.toml", "toml");
        e.range = Some(PositionRange {
            start: Position {
                line: 17,
                column: 21,
            },
            end: Position {
                line: 18,
                column: 5,
            },
        });
        e.snippet = "ttps://example\nchecksum = \"x\"".into();
        let s = session(vec![e]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("T".into()),
            repo_name: None,
            git: None,
        });
        assert!(
            out.contains("                    ttps://example\nchecksum = \"x\""),
            "got:\n{}",
            out
        );
    }

    #[test]
    fn long_snippet_truncated_with_marker() {
        let mut e = snippet_entry("e", "big.rs", "rust");
        let lines: Vec<String> = (1..=60).map(|i| format!("line {}", i)).collect();
        e.snippet = lines.join("\n");
        e.range = Some(PositionRange {
            start: Position { line: 1, column: 1 },
            end: Position {
                line: 60,
                column: 8,
            },
        });
        let s = session(vec![e]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("T".into()),
            repo_name: None,
            git: None,
        });
        assert!(out.contains("line 30\n"), "head must be present: {}", out);
        assert!(
            !out.contains("line 31\n"),
            "elided portion must be gone: {}",
            out
        );
        assert!(
            out.contains("[... 30 more lines elided ...]"),
            "expected elision marker, got: {}",
            out
        );
    }

    #[test]
    fn file_entry_renders_without_range_or_code() {
        let mut e = snippet_entry("e", "README.md", "markdown");
        e.range = None;
        e.snippet = String::new();
        e.comment = "markdown 文件不要添加截断换行。".into();
        let s = session(vec![e]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("T".into()),
            repo_name: None,
            git: None,
        });
        assert!(
            out.contains("## [1] README.md\n\nmarkdown"),
            "got:\n{}",
            out
        );
        assert!(!out.contains("```"), "should have no code fence: {}", out);
    }

    #[test]
    fn project_entry_renders_with_no_file_and_no_code() {
        let e = CommentEntry {
            id: "p".into(),
            file: None,
            language: String::new(),
            range: None,
            snippet: String::new(),
            comment: "添加 prompt 快捷键触发时如果没有选区就添加项目级的修改。".into(),
            created_at: "2026-05-08T00:00:00.000Z".into(),
            git: None,
        };
        let s = session(vec![e]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("T".into()),
            repo_name: None,
            git: None,
        });
        assert!(out.contains("## [1]\n\n添加"), "got:\n{}", out);
        assert!(!out.contains("```"), "should have no code fence: {}", out);
    }

    #[test]
    fn single_line_snippet_does_not_indent_even_with_offset_start_col() {
        let mut e = snippet_entry("e", "a.rs", "rust");
        e.snippet = "println!(\"hi\")".into();
        e.range = Some(PositionRange {
            start: Position { line: 5, column: 9 },
            end: Position {
                line: 5,
                column: 23,
            },
        });
        let s = session(vec![e]);
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("T".into()),
            repo_name: None,
            git: None,
        });
        assert!(
            out.contains("\n```rust\nprintln!(\"hi\")\n```"),
            "single-line snippet should not get leading indent, got:\n{}",
            out
        );
    }

    #[test]
    fn workspace_root_basename_used_when_repo_name_omitted() {
        let mut s = session(vec![]);
        s.workspace_root = "/var/projects/my-cool-app".into();
        let out = render(RenderArgs {
            session: &s,
            template: TEMPLATE,
            timestamp: Some("T".into()),
            repo_name: None,
            git: None,
        });
        assert!(out.contains("Repository: my-cool-app"), "got: {}", out);
    }
}
