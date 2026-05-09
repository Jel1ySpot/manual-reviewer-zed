use std::path::Path;

use anyhow::Result;
use manual_reviewer_core::entry::EntryKind;
use manual_reviewer_core::Store;

pub fn run(workspace: &Path, json: bool) -> Result<()> {
    let store = Store::open(workspace)?;
    if json {
        println!("{}", serde_json::to_string_pretty(store.session())?);
        return Ok(());
    }
    if store.count() == 0 {
        println!("(no entries)");
        return Ok(());
    }
    for (i, e) in store.entries().iter().enumerate() {
        let preview = e
            .comment
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(80)
            .collect::<String>();
        let header = match e.kind() {
            EntryKind::Snippet => format!(
                "{}<{}>",
                e.file.as_deref().unwrap_or(""),
                e.range.as_ref().unwrap().format()
            ),
            EntryKind::File => {
                format!("{} (whole file)", e.file.as_deref().unwrap_or(""))
            }
            EntryKind::Project => "(project-level)".to_string(),
        };
        println!(
            "[#{:>2}] {}  {}\n      id={}",
            i + 1,
            header,
            preview,
            e.id
        );
    }
    println!("\n{} entries.", store.count());
    Ok(())
}
