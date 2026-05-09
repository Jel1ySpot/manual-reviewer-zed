use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use manual_reviewer_core::entry::{new_id, now_iso, CommentEntry, Position, PositionRange};
use manual_reviewer_core::{git, Store};

use crate::term_input::read_multiline_comment;

pub fn run(workspace: &Path, location: &str, message: Option<String>) -> Result<()> {
    let parsed = parse_location(location)
        .with_context(|| format!("parse location {:?}", location))?;
    let abs_file = if parsed.file.is_absolute() {
        parsed.file.clone()
    } else {
        workspace.join(&parsed.file)
    };
    let snippet = read_snippet(&abs_file, &parsed.range)
        .with_context(|| format!("read snippet from {}", abs_file.display()))?;
    let language = guess_language(&abs_file);

    let comment = match message {
        Some(m) if !m.trim().is_empty() => m.trim().to_string(),
        _ => read_multiline_comment("Write your comment:")?,
    };
    if comment.is_empty() {
        return Err(anyhow!("Empty comment — entry not added."));
    }

    let mut store = Store::open(workspace)?;
    let entry = CommentEntry {
        id: new_id(),
        file: Some(parsed.file.to_string_lossy().into_owned()),
        language,
        range: Some(parsed.range),
        snippet,
        comment,
        created_at: now_iso(),
        git: git::capture(workspace),
    };
    let new_count = store.count() + 1;
    store.add(entry)?;
    println!("Added entry #{}. Total: {} entries.", new_count, store.count());
    Ok(())
}

#[derive(Debug)]
struct ParsedLocation {
    file: PathBuf,
    range: PositionRange,
}

/// Parse `path/to/file:startLine:startCol-endLine:endCol`.
fn parse_location(input: &str) -> Result<ParsedLocation> {
    // Split off the trailing range. The range is always after the *last* `:` group.
    // Strategy: find the rightmost run that matches `<num>:<num>-<num>:<num>` and
    // treat everything before it as the file path.
    let bytes = input.as_bytes();
    // Find the last '-' that has digits-and-colons on both sides.
    for i in (0..bytes.len()).rev() {
        if bytes[i] != b'-' {
            continue;
        }
        let (head, tail) = input.split_at(i);
        let tail = &tail[1..]; // skip '-'
        // tail must be `endLine:endCol`
        if !tail.contains(':') {
            continue;
        }
        // head must end with `:startLine:startCol` after some `path:`
        let last_colon = head.rfind(':');
        if last_colon.is_none() {
            continue;
        }
        let last_colon = last_colon.unwrap();
        let before_last = &head[..last_colon];
        let second_last = before_last.rfind(':');
        if second_last.is_none() {
            continue;
        }
        let second_last = second_last.unwrap();
        let file_str = &head[..second_last];
        let start_line_str = &head[second_last + 1..last_colon];
        let start_col_str = &head[last_colon + 1..];

        let mut tail_parts = tail.splitn(2, ':');
        let end_line_str = tail_parts.next().unwrap_or("");
        let end_col_str = tail_parts.next().unwrap_or("");

        if let (Ok(sl), Ok(sc), Ok(el), Ok(ec)) = (
            start_line_str.parse::<u32>(),
            start_col_str.parse::<u32>(),
            end_line_str.parse::<u32>(),
            end_col_str.parse::<u32>(),
        ) {
            return Ok(ParsedLocation {
                file: PathBuf::from(file_str),
                range: PositionRange {
                    start: Position { line: sl, column: sc },
                    end: Position { line: el, column: ec },
                },
            });
        }
    }
    Err(anyhow!(
        "expected `file:startLine:startCol-endLine:endCol`, got {:?}",
        input
    ))
}

fn read_snippet(file: &Path, range: &PositionRange) -> Result<String> {
    let bytes = fs::read(file)?;
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    if range.start.line as usize == 0 || range.end.line as usize > lines.len() {
        return Err(anyhow!(
            "range {} out of bounds (file has {} lines)",
            range.format(),
            lines.len()
        ));
    }
    if range.start.line == range.end.line {
        let line = lines[range.start.line as usize - 1];
        let chars: Vec<char> = line.chars().collect();
        let s = (range.start.column as usize).saturating_sub(1).min(chars.len());
        let e = (range.end.column as usize).saturating_sub(1).min(chars.len());
        return Ok(chars[s..e].iter().collect());
    }
    let mut buf = String::new();
    let first = lines[range.start.line as usize - 1];
    let first_chars: Vec<char> = first.chars().collect();
    let s = (range.start.column as usize).saturating_sub(1).min(first_chars.len());
    buf.push_str(&first_chars[s..].iter().collect::<String>());
    buf.push('\n');
    for line in &lines[range.start.line as usize..range.end.line as usize - 1] {
        buf.push_str(line);
        buf.push('\n');
    }
    let last = lines[range.end.line as usize - 1];
    let last_chars: Vec<char> = last.chars().collect();
    let e = (range.end.column as usize).saturating_sub(1).min(last_chars.len());
    buf.push_str(&last_chars[..e].iter().collect::<String>());
    Ok(buf)
}

fn guess_language(file: &Path) -> String {
    file.extension()
        .and_then(|e| e.to_str())
        .map(|s| match s {
            "ts" => "typescript",
            "tsx" => "typescriptreact",
            "js" => "javascript",
            "jsx" => "javascriptreact",
            "rs" => "rust",
            "py" => "python",
            "go" => "go",
            "java" => "java",
            "kt" => "kotlin",
            "rb" => "ruby",
            "md" => "markdown",
            "yml" | "yaml" => "yaml",
            "toml" => "toml",
            "json" => "json",
            "sh" | "bash" => "shellscript",
            "zsh" => "shellscript",
            other => other,
        })
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_path() {
        let p = parse_location("src/foo.rs:1:1-3:8").unwrap();
        assert_eq!(p.file, PathBuf::from("src/foo.rs"));
        assert_eq!(p.range.start.line, 1);
        assert_eq!(p.range.end.column, 8);
    }

    #[test]
    fn parses_path_with_colons_in_name() {
        // unusual but legal on some filesystems (and Windows drive letters)
        let p = parse_location("src/weird:name.txt:5:2-10:4").unwrap();
        assert_eq!(p.file, PathBuf::from("src/weird:name.txt"));
        assert_eq!(p.range.start.line, 5);
        assert_eq!(p.range.end.line, 10);
    }

    #[test]
    fn rejects_missing_range() {
        assert!(parse_location("src/foo.rs").is_err());
        assert!(parse_location("src/foo.rs:1").is_err());
        assert!(parse_location("src/foo.rs:1:1").is_err());
    }

    #[test]
    fn guess_language_known_exts() {
        assert_eq!(guess_language(Path::new("a.ts")), "typescript");
        assert_eq!(guess_language(Path::new("b.rs")), "rust");
        assert_eq!(guess_language(Path::new("c.unknown")), "unknown");
        assert_eq!(guess_language(Path::new("noext")), "");
    }
}
