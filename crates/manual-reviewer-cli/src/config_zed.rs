use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

/// Tasks the CLI ships with. These are also kept in sync with
/// `manual-reviewer-zed/.zed/tasks.json` (used as the per-project template).
const TASKS_JSON: &str = include_str!("../../../.zed/tasks.json");

/// Reference keymap. The keymap file in `.zed/keymap.json` is for documentation
/// only — Zed doesn't load project-level keymaps. We embed the same bindings
/// here and merge them into the user's global keymap.
const KEYMAP_BINDINGS_JSON: &str = r#"{
  "context": "Editor && !VimWaiting",
  "bindings": {
    "cmd-\"":  ["task::Spawn", { "task_name": "Manual Reviewer: Add for selection" }],
    "ctrl-\"": ["task::Spawn", { "task_name": "Manual Reviewer: Add for selection" }],
    "cmd-I":   ["task::Spawn", { "task_name": "Manual Reviewer: Export prompt" }],
    "ctrl-I":  ["task::Spawn", { "task_name": "Manual Reviewer: Export prompt" }]
  }
}"#;

const TASK_LABEL_PREFIX: &str = "Manual Reviewer: ";

pub fn run(dry_run: bool) -> Result<()> {
    let zed_dir = config_dir()?;
    fs::create_dir_all(&zed_dir).with_context(|| format!("create {}", zed_dir.display()))?;

    let tasks_path = zed_dir.join("tasks.json");
    let keymap_path = zed_dir.join("keymap.json");

    let tasks_change = merge_tasks(&tasks_path)?;
    let keymap_change = merge_keymap(&keymap_path)?;

    if dry_run {
        println!("--- {} ---", tasks_path.display());
        println!("{}\n", tasks_change.preview);
        println!("--- {} ---", keymap_path.display());
        println!("{}\n", keymap_change.preview);
        println!("(dry run; no files written)");
        return Ok(());
    }

    if let Some(content) = tasks_change.to_write {
        write_with_backup(&tasks_path, &content)?;
        println!("Wrote {} ({})", tasks_path.display(), tasks_change.summary);
    } else {
        println!("Skipped {} ({})", tasks_path.display(), tasks_change.summary);
    }
    if let Some(content) = keymap_change.to_write {
        write_with_backup(&keymap_path, &content)?;
        println!("Wrote {} ({})", keymap_path.display(), keymap_change.summary);
    } else {
        println!(
            "Skipped {} ({})",
            keymap_path.display(),
            keymap_change.summary
        );
    }
    Ok(())
}

fn config_dir() -> Result<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".config").join("zed"));
    }
    Err(anyhow!("$HOME is not set; cannot locate ~/.config/zed"))
}

struct ChangeReport {
    to_write: Option<String>,
    summary: String,
    preview: String,
}

fn merge_tasks(path: &PathBuf) -> Result<ChangeReport> {
    let our_tasks: Vec<Value> = serde_json::from_str(TASKS_JSON)
        .context("embedded tasks.json failed to parse")?;
    let our_labels: Vec<String> = our_tasks
        .iter()
        .filter_map(|t| t.get("label").and_then(|l| l.as_str()).map(String::from))
        .collect();

    let existing_text = fs::read_to_string(path).ok();
    let mut existing: Vec<Value> = match &existing_text {
        Some(text) if !text.trim().is_empty() => match parse_jsonc_array(text) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ChangeReport {
                    to_write: None,
                    summary: format!(
                        "existing file failed to parse ({}); leaving untouched",
                        e
                    ),
                    preview: text.clone(),
                });
            }
        },
        _ => Vec::new(),
    };

    let mut added = 0usize;
    let mut replaced = 0usize;
    for new_task in &our_tasks {
        let new_label = new_task
            .get("label")
            .and_then(|l| l.as_str())
            .unwrap_or("");
        let pos = existing
            .iter()
            .position(|t| t.get("label").and_then(|l| l.as_str()) == Some(new_label));
        match pos {
            Some(i) => {
                if existing[i] != *new_task {
                    existing[i] = new_task.clone();
                    replaced += 1;
                }
            }
            None => {
                existing.push(new_task.clone());
                added += 1;
            }
        }
    }
    let _ = our_labels;
    let new_text = serde_json::to_string_pretty(&existing)? + "\n";
    let summary = format!("added {}, updated {}", added, replaced);
    let to_write = if Some(&new_text) == existing_text.as_ref() {
        None
    } else {
        Some(new_text.clone())
    };
    Ok(ChangeReport {
        to_write,
        summary,
        preview: new_text,
    })
}

fn merge_keymap(path: &PathBuf) -> Result<ChangeReport> {
    let our_block: Value = serde_json::from_str(KEYMAP_BINDINGS_JSON)
        .context("embedded keymap block failed to parse")?;
    let our_context = our_block
        .get("context")
        .and_then(|v| v.as_str())
        .unwrap_or("Editor")
        .to_string();
    let our_bindings = our_block
        .get("bindings")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("embedded keymap missing bindings"))?
        .clone();

    let existing_text = fs::read_to_string(path).ok();
    let mut existing: Vec<Value> = match &existing_text {
        Some(text) if !text.trim().is_empty() => match parse_jsonc_array(text) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ChangeReport {
                    to_write: None,
                    summary: format!(
                        "existing file failed to parse ({}); leaving untouched",
                        e
                    ),
                    preview: text.clone(),
                });
            }
        },
        _ => Vec::new(),
    };

    // First pass: strip ANY existing binding (in any context block) that
    // resolves to a Manual Reviewer task — this cleans up bindings left over
    // from previous installs (e.g. a renamed key, or `cmd-alt-e`).
    let mut stripped = 0usize;
    for block in existing.iter_mut() {
        if let Some(obj) = block.as_object_mut() {
            if let Some(bindings) = obj.get_mut("bindings").and_then(|v| v.as_object_mut()) {
                let to_remove: Vec<String> = bindings
                    .iter()
                    .filter_map(|(k, v)| {
                        if binding_targets_manual_reviewer(v) {
                            Some(k.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                for k in to_remove {
                    bindings.remove(&k);
                    stripped += 1;
                }
            }
        }
    }
    // Drop any context blocks that ended up empty after the cleanup.
    existing.retain(|b| match b.get("bindings").and_then(|v| v.as_object()) {
        Some(m) => !m.is_empty(),
        None => true,
    });

    // Locate an existing block with the same `context`. If found, merge our
    // bindings into it; otherwise append our block as-is.
    let mut added_keys = 0usize;
    let mut replaced_keys = 0usize;
    let pos = existing
        .iter()
        .position(|b| b.get("context").and_then(|v| v.as_str()) == Some(our_context.as_str()));
    match pos {
        Some(i) => {
            let block = existing[i]
                .as_object_mut()
                .ok_or_else(|| anyhow!("expected object at keymap[{}]", i))?;
            let bindings_entry = block
                .entry("bindings".to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            let bindings_map = bindings_entry
                .as_object_mut()
                .ok_or_else(|| anyhow!("bindings is not an object at keymap[{}]", i))?;
            for (k, v) in our_bindings.iter() {
                match bindings_map.get(k) {
                    Some(existing_v) if existing_v == v => {}
                    Some(_) => {
                        bindings_map.insert(k.clone(), v.clone());
                        replaced_keys += 1;
                    }
                    None => {
                        bindings_map.insert(k.clone(), v.clone());
                        added_keys += 1;
                    }
                }
            }
        }
        None => {
            existing.push(our_block.clone());
            added_keys = our_bindings.len();
        }
    }

    let new_text = serde_json::to_string_pretty(&existing)? + "\n";
    let summary = {
        let cleanup = if stripped > 0 {
            format!(", cleaned up {} stale Manual Reviewer binding(s)", stripped)
        } else {
            String::new()
        };
        if pos.is_some() {
            format!(
                "merged into existing {} block: {} added, {} updated{}",
                our_context, added_keys, replaced_keys, cleanup
            )
        } else {
            format!(
                "appended new {} block ({} bindings){}",
                our_context, added_keys, cleanup
            )
        }
    };
    let to_write = if Some(&new_text) == existing_text.as_ref() {
        None
    } else {
        Some(new_text.clone())
    };
    Ok(ChangeReport {
        to_write,
        summary,
        preview: new_text,
    })
}

/// Returns true if a keymap binding value resolves to a `task::Spawn` of a
/// task whose name starts with "Manual Reviewer: ". Used to find stale
/// bindings to clean up before re-installing.
fn binding_targets_manual_reviewer(v: &Value) -> bool {
    let arr = match v.as_array() {
        Some(a) => a,
        None => return false,
    };
    if arr.len() < 2 {
        return false;
    }
    let action = arr[0].as_str().unwrap_or("");
    if action != "task::Spawn" {
        return false;
    }
    let payload = match arr[1].as_object() {
        Some(o) => o,
        None => return false,
    };
    payload
        .get("task_name")
        .and_then(|v| v.as_str())
        .map(|s| s.starts_with(TASK_LABEL_PREFIX))
        .unwrap_or(false)
}

fn write_with_backup(path: &PathBuf, content: &str) -> Result<()> {
    if path.exists() {
        let backup = path.with_extension(
            path.extension()
                .and_then(|e| e.to_str())
                .map(|e| format!("{}.bak", e))
                .unwrap_or_else(|| "bak".into()),
        );
        let _ = fs::copy(path, &backup);
    }
    fs::write(path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Parse a JSONC-style array. Strips `// ...` line comments and `/* ... */`
/// block comments, then trailing commas, before deferring to `serde_json`.
fn parse_jsonc_array(text: &str) -> Result<Vec<Value>> {
    let stripped = strip_jsonc(text);
    let v: Value =
        serde_json::from_str(&stripped).with_context(|| "parse JSON after comment stripping")?;
    match v {
        Value::Array(a) => Ok(a),
        _ => Err(anyhow!("expected JSON array at top level")),
    }
}

fn strip_jsonc(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    let mut in_string = false;
    let mut escape = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            out.push(c as char);
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'/' {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
        }
        out.push(c as char);
        i += 1;
    }
    // Strip trailing commas before `]` or `}`.
    let mut cleaned = String::with_capacity(out.len());
    let chars: Vec<char> = out.chars().collect();
    let mut j = 0;
    while j < chars.len() {
        let c = chars[j];
        if c == ',' {
            let mut k = j + 1;
            while k < chars.len() && chars[k].is_whitespace() {
                k += 1;
            }
            if k < chars.len() && (chars[k] == ']' || chars[k] == '}') {
                j += 1;
                continue;
            }
        }
        cleaned.push(c);
        j += 1;
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_jsonc_removes_line_and_block_comments() {
        let input = r#"// header
[
  // task A
  { "label": "A" /* inline */ },
  { "label": "B" },
]
"#;
        let arr = parse_jsonc_array(input).unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["label"], "A");
    }

    #[test]
    fn strip_jsonc_preserves_strings_with_slashes() {
        let input = r#"[ { "url": "http://x/y" } ]"#;
        let arr = parse_jsonc_array(input).unwrap();
        assert_eq!(arr[0]["url"], "http://x/y");
    }
}
