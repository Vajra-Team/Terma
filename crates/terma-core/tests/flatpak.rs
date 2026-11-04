use std::path::PathBuf;

use terma_core::flatpak::{self, FlatpakApp, FlatpakDuplicate, Installation};
use terma_core::programs::{Program, UpdateState};
use terma_core::runner::FakeRunner;
use terma_core::search::Source;

const LIST: &str = include_str!("fixtures/programs/flatpak-list.txt");

fn program(name: &str) -> Program {
    Program {
        name: name.into(),
        icon: None,
        source: Source::Repo { repo: String::new() },
        update: UpdateState::None,
        desktop_file: PathBuf::from("/usr/share/applications/x.desktop"),
        package: Some(name.to_lowercase()),
    }
}

#[test]
fn list_apps_parses_real_output() {
    let runner = FakeRunner::new().on(&["flatpak", "list"], 0, LIST);
    let apps = flatpak::list_apps(&runner);
    assert!(apps.contains(&FlatpakApp { id: "com.discordapp.Discord".into(), name: "Discord".into(), installation: Installation::System }));
    assert_eq!(apps.len(), LIST.lines().filter(|l| !l.trim().is_empty()).count());
}

#[test]
fn list_apps_without_flatpak_is_empty() {
    assert!(flatpak::list_apps(&FakeRunner::new()).is_empty());
}

#[test]
fn flatpak_duplicate_discord() {
    let runner = FakeRunner::new().on(&["flatpak", "list"], 0, LIST);
    let dups = flatpak::duplicates(&[program("Discord"), program("Steam")], &flatpak::list_apps(&runner));
    assert_eq!(
        dups,
        vec![FlatpakDuplicate { name: "Discord".into(), flatpak_id: "com.discordapp.Discord".into(), installation: Installation::System }]
    );
}

#[test]
fn flatpak_uninstall_argv() {
    let runner = FakeRunner::new().on(&["flatpak", "uninstall"], 0, "");
    let dup = FlatpakDuplicate { name: "Discord".into(), flatpak_id: "com.discordapp.Discord".into(), installation: Installation::System };

    flatpak::uninstall(&runner, &dup).unwrap();

    assert_eq!(runner.calls()[0].argv, ["flatpak", "uninstall", "--noninteractive", "-y", "--system", "com.discordapp.Discord"]);
}
