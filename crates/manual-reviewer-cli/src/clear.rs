use std::path::Path;

use anyhow::Result;
use manual_reviewer_core::Store;

pub fn run(workspace: &Path, archive: bool) -> Result<()> {
    let mut store = Store::open(workspace)?;
    let n = store.count();
    if n == 0 {
        println!("Already empty.");
        return Ok(());
    }
    store.clear(archive)?;
    if archive {
        println!("Cleared {} entries (archived to .mreview/archive/).", n);
    } else {
        println!("Cleared {} entries.", n);
    }
    Ok(())
}
