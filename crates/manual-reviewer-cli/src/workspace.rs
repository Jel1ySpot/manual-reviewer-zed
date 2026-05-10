use std::env;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use manual_reviewer_core::store::find_workspace_root;

/// Resolve the workspace root, in priority order:
/// 1. explicit `--workspace` flag (env: MREVIEW_WORKSPACE_ROOT)
/// 2. $MREVIEW_WORKTREE_ROOT (set by Zed task)
/// 3. nearest ancestor with `.git`, walking from current working directory
/// 4. fallback: current working directory itself
pub fn resolve(explicit: Option<String>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
        return Err(anyhow!("workspace path {:?} does not exist", p));
    }
    if let Ok(zed_root) = env::var("MREVIEW_WORKTREE_ROOT") {
        if !zed_root.is_empty() {
            let p = PathBuf::from(zed_root);
            if p.exists() {
                return Ok(p);
            }
        }
    }
    let cwd = env::current_dir()?;
    Ok(find_workspace_root(&cwd))
}
