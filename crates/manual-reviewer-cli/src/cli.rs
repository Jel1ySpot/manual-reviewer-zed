use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "mreview",
    version,
    about = "Manual Reviewer — leave structured review comments on selected code regions and export them as a coding-agent prompt.",
    long_about = None,
)]
pub struct Cli {
    /// Override the workspace root. Defaults to $MREVIEW_ZED_WORKTREE_ROOT,
    /// then the nearest ancestor `.git`, then the current directory.
    #[arg(long, global = true, env = "MREVIEW_WORKSPACE_ROOT")]
    pub workspace: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add a new comment entry.
    Add(AddArgs),
    /// List all entries in the current session.
    List(ListArgs),
    /// Remove a single entry by id.
    Remove(RemoveArgs),
    /// Clear the current session (with optional archive).
    Clear(ClearArgs),
    /// Render the prompt and write it to .mreview/PROMPT.md.
    Export(ExportArgs),
    /// Merge the Manual Reviewer tasks + keymap bindings into the user's
    /// global Zed config (~/.config/zed/{tasks,keymap}.json).
    ConfigZed(ConfigZedArgs),
}

#[derive(Debug, Args)]
pub struct ConfigZedArgs {
    /// Print the merged result without writing anything to disk.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// `file:startLine:startCol-endLine:endCol`. Required unless --from-zed-task.
    #[arg(value_name = "LOCATION")]
    pub location: Option<String>,

    /// Read selection + position from $MREVIEW_ZED_* environment variables
    /// (set by the Zed task in `.zed/tasks.json`).
    #[arg(long, conflicts_with = "location")]
    pub from_zed_task: bool,

    /// One-line comment. If omitted, you'll be prompted in the terminal
    /// (multi-line; finish with empty line or Ctrl-D).
    #[arg(short, long, value_name = "TEXT")]
    pub message: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Print entries as JSON instead of a human-friendly summary.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// Entry to remove. Can be: a UUID, a 1-based index (`3`), a comma list
    /// (`1,3,5`), an inclusive range (`2-4`), or any combination (`1,3-5,8`).
    pub spec: String,
}

#[derive(Debug, Args)]
pub struct ClearArgs {
    /// Also write a timestamped archive copy to .mreview/archive/ first.
    #[arg(long)]
    pub archive: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Output path (default: .mreview/PROMPT.md inside the workspace).
    #[arg(long, value_name = "PATH")]
    pub out: Option<String>,

    /// Format. Defaults to markdown.
    #[arg(long, value_name = "FMT", default_value = "markdown")]
    pub format: String,

    /// Skip copying to clipboard (default: copy).
    #[arg(long)]
    pub no_copy: bool,

    /// Skip opening with `open` / `xdg-open` (default: open).
    #[arg(long)]
    pub no_open: bool,

    /// Skip writing to disk (stdout-only). Useful when piping.
    #[arg(long)]
    pub no_write: bool,
}
