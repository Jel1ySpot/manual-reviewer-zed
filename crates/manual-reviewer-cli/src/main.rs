use anyhow::Result;
use clap::Parser;

mod add_manual;
mod add_zed;
mod clear;
mod cli;
mod export;
mod config_zed;
mod list;
mod remove;
mod term_input;
mod workspace;

use cli::{Cli, Command};

fn main() -> Result<()> {
    let args = Cli::parse();

    // `config-zed` doesn't need a workspace.
    if let Command::ConfigZed(a) = &args.command {
        return config_zed::run(a.dry_run);
    }

    let workspace = workspace::resolve(args.workspace.clone())?;

    match args.command {
        Command::Add(a) => {
            if a.from_zed_task {
                add_zed::run(&workspace, a.message)
            } else if let Some(loc) = a.location {
                add_manual::run(&workspace, &loc, a.message)
            } else {
                Err(anyhow::anyhow!(
                    "missing LOCATION (or use --from-zed-task)"
                ))
            }
        }
        Command::List(a) => list::run(&workspace, a.json),
        Command::Remove(a) => remove::run(&workspace, &a.spec),
        Command::Clear(a) => clear::run(&workspace, a.archive),
        Command::Export(a) => export::run(&workspace, &a),
        Command::ConfigZed(_) => unreachable!("handled above"),
    }
}
