//! Zeigt, was Terma in einer AppImage sieht: `cargo run -p terma-core --example inspect -- <datei> [--update]`
//! Mit `--update` wird zusätzlich online nach einer neueren Version gesucht.

fn main() {
    let path = std::env::args().nth(1).expect("Pfad zur AppImage fehlt");
    match terma_core::appimage::inspect(std::path::Path::new(&path)) {
        Ok(info) if std::env::args().any(|a| a == "--update") => {
            match terma_core::appimage_update::check(&info, &terma_core::http::UreqHttp::new()) {
                Ok(Some(remote)) => println!("Update: {} (SHA-1 {})", remote.url, remote.sha1),
                Ok(None) => println!("Kein Update (aktuell oder keine Update-Information)"),
                Err(e) => println!("Fehler: {e}"),
            }
        }
        Ok(info) => println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "path": info.path,
                "name": info.name,
                "desktop_file_name": info.desktop_file_name,
                "version": info.version,
                "update_info": info.update_info,
                "self_updating": info.self_updating,
                "icon": info.icon.as_ref().map(|i| format!("{} ({} Bytes)", i.ext, i.bytes.len())),
                "exec": info.desktop.get("Exec"),
                "mime_types": info.desktop.mime_types(),
            }))
            .unwrap()
        ),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
