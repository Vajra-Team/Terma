//! Datei-Fenster ohne Oberfläche: `cargo run -p terma-core --example file -- <datei>`

fn main() {
    let path = std::env::args().nth(1).expect("Pfad fehlt");
    let terma = terma_core::ops::Terma::new(terma_core::dirs::UserDirs::from_env());
    match terma.inspect_file(std::path::Path::new(&path)) {
        Ok(i) => println!(
            "{:?} „{}“ {:?} → {}",
            i.kind,
            i.app_name,
            i.app_version,
            i.source_match.map(|c| format!("{} ({:?})", c.name, c.source)).unwrap_or_else(|| "kein Treffer, AppImage einrichten".into())
        ),
        Err(e) => println!("Fehler: {e}"),
    }
}
