use std::path::Path;
use std::process::Command;

use crate::entry::GitInfo;

pub fn capture(workspace_root: &Path) -> Option<GitInfo> {
    let sha = run_git(workspace_root, &["rev-parse", "HEAD"])?;
    let branch =
        run_git(workspace_root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let dirty = is_dirty(workspace_root);
    Some(GitInfo {
        sha,
        branch,
        dirty,
    })
}

fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(cwd).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn is_dirty(cwd: &Path) -> bool {
    let out = match Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(cwd)
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };
    if !out.status.success() {
        return false;
    }
    !out.stdout.is_empty()
}
