use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use manual_reviewer_core::render::{render, RenderArgs, DEFAULT_TEMPLATE};
use manual_reviewer_core::{git, Store};

use crate::cli::ExportArgs;

const DEFAULT_OUT: &str = ".mreview/PROMPT.md";

pub fn run(workspace: &Path, args: &ExportArgs) -> Result<()> {
    let store = Store::open(workspace)?;
    let format = args.format.to_lowercase();
    let body = match format.as_str() {
        "markdown" | "md" => {
            let g = git::capture(workspace);
            let repo = workspace
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            render(RenderArgs {
                session: store.session(),
                template: DEFAULT_TEMPLATE,
                repo_name: repo.as_deref(),
                git: g.as_ref(),
                timestamp: None,
            })
        }
        "json" => serde_json::to_string_pretty(store.session())?,
        other => return Err(anyhow!("unknown format {:?}", other)),
    };

    if args.no_write {
        print!("{}", body);
        return Ok(());
    }

    let out_path = match &args.out {
        Some(p) => PathBuf::from(p),
        None => workspace.join(DEFAULT_OUT),
    };
    let out_path = if out_path.is_absolute() {
        out_path
    } else {
        workspace.join(out_path)
    };

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create parent dir {}", parent.display()))?;
    }

    // Archive previous file if exists.
    if out_path.exists() {
        let _ = store.archive_prompt_file(&out_path);
    }
    fs::write(&out_path, &body).with_context(|| format!("write {}", out_path.display()))?;

    let do_copy = !args.no_copy;
    let do_open = !args.no_open;
    if do_copy {
        if let Err(e) = copy_to_clipboard(&body) {
            eprintln!("warning: clipboard copy failed: {}", e);
        }
    }
    let opened_with = if do_open {
        open_path(&out_path, args.editor.as_deref())
    } else {
        None
    };

    println!(
        "Exported {} entr{} → {}{}{}",
        store.count(),
        if store.count() == 1 { "y" } else { "ies" },
        rel_for_print(workspace, &out_path),
        if do_copy {
            " · copied to clipboard"
        } else {
            ""
        },
        match opened_with {
            Some(cmd) => format!(" · opened with {}", cmd),
            None => String::new(),
        },
    );
    Ok(())
}

fn copy_to_clipboard(body: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(body.to_string())?;
    Ok(())
}

/// Try the openers in priority order, returning the first that successfully
/// spawned (or `None` if all failed). Order:
/// 1. `--editor` flag (when supplied)
/// 2. `$EDITOR` environment variable
/// 3. Built-in fallback list: `zed`, `code`, `cursor`, `subl`, `xdg-open`, `open`
///
/// Failure is silent — the export already succeeded; opening is best-effort.
fn open_path(p: &Path, override_editor: Option<&str>) -> Option<String> {
    if let Some(spec) = override_editor {
        if let Some(used) = try_spawn_string(spec, p) {
            return Some(used);
        }
    }

    if let Ok(spec) = std::env::var("EDITOR") {
        if let Some(used) = try_spawn_string(spec.trim(), p) {
            return Some(used);
        }
    }

    const OPENERS: &[&str] = &["zed", "code", "cursor", "subl", "xdg-open", "open"];
    for cmd in OPENERS {
        if Command::new(cmd).arg(p).spawn().is_ok() {
            return Some((*cmd).to_string());
        }
    }
    None
}

/// Parse `spec` as a command-line (whitespace-split), append `p`, spawn.
/// Returns `Some(spec.to_string())` on success.
fn try_spawn_string(spec: &str, p: &Path) -> Option<String> {
    if spec.is_empty() {
        return None;
    }
    let mut parts = spec.split_whitespace();
    let prog = parts.next()?;
    let mut cmd = Command::new(prog);
    for a in parts {
        cmd.arg(a);
    }
    cmd.arg(p);
    if cmd.spawn().is_ok() {
        Some(spec.to_string())
    } else {
        None
    }
}

fn rel_for_print(workspace: &Path, target: &Path) -> String {
    target
        .strip_prefix(workspace)
        .map(|r| r.to_string_lossy().into_owned())
        .unwrap_or_else(|_| target.to_string_lossy().into_owned())
}
