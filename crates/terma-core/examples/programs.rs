//! Zeigt „Deine Programme“ und doppelte Flatpaks so, wie Terma sie sieht: `cargo run -p terma-core --example programs`

use std::collections::HashSet;
use std::sync::Arc;

use terma_core::dirs::UserDirs;
use terma_core::pacman::PacmanDb;
use terma_core::programs;
use terma_core::runner::SystemRunner;

fn main() {
    let dirs = UserDirs::from_env();
    let runner = Arc::new(SystemRunner);
    let db = PacmanDb::new(&dirs, runner.clone());
    let inputs = programs::gather(&db, &dirs, runner.as_ref(), HashSet::new()).expect("Programme lesen");
    let list = programs::assemble(inputs, &|icon| programs::resolve_icon(icon, &dirs));
    for p in &list {
        println!("{:<34} {:<14} Icon: {}", p.name, format!("{:?}", p.source).split_whitespace().next().unwrap_or(""), if p.icon.is_some() { "ja" } else { "nein" });
    }
    println!("{} Programme", list.len());
    for d in terma_core::flatpak::duplicates(&list, &terma_core::flatpak::list_apps(runner.as_ref())) {
        println!("Doppelt: {} ({})", d.name, d.flatpak_id);
    }
}
