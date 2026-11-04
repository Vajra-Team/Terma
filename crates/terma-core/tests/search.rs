use std::collections::HashSet;

use terma_core::aur::AurPackage;
use terma_core::error::TermaError;
use terma_core::pacman::RepoPackage;
use terma_core::search::{self, display_name, normalize, query_terms, rank, Source};

fn repo_discord() -> Vec<RepoPackage> {
    serde_json::from_str(include_str!("fixtures/search/repo-discord.json")).unwrap()
}

fn aur(fixture: &str) -> Vec<AurPackage> {
    serde_json::from_str(fixture).unwrap()
}

fn aur_pkg(name: &str, votes: u32) -> AurPackage {
    AurPackage {
        name: name.into(),
        package_base: name.into(),
        version: "1-1".into(),
        description: String::new(),
        url: None,
        num_votes: votes,
        popularity: 1.0,
        maintainer: Some("x".into()),
        first_submitted: 0,
        last_modified: 0,
        depends: vec![],
        make_depends: vec![],
        provides: vec![],
    }
}

fn repo_pkg(name: &str, desktop: Option<bool>, installed: Option<&str>) -> RepoPackage {
    RepoPackage {
        name: name.into(),
        version: "1-1".into(),
        description: String::new(),
        repo: "extra".into(),
        download_size: 1,
        installed_version: installed.map(Into::into),
        has_desktop_entry: desktop,
        url: None,
    }
}

#[test]
fn query_terms_variants() {
    assert_eq!(query_terms("T3 Code"), vec!["t3code", "t3-code"]);
    assert_eq!(query_terms("discord"), vec!["discord"]);
    assert_eq!(query_terms("  Discord  "), vec!["discord"]);
}

#[test]
fn display_name_cases() {
    assert_eq!(display_name("discord-canary"), "Discord Canary");
    assert_eq!(display_name("t3code-bin"), "T3code");
    assert_eq!(display_name("keepassxc"), "Keepassxc");
}

#[test]
fn normalize_strips_suffixes() {
    for name in ["t3code-bin", "T3 Code", "t3-code", "T3 Code (Beta)", "t3code-git", "T3-Code-AppImage"] {
        assert_eq!(normalize(name), "t3code", "{name}");
    }
}

#[test]
fn discord_recommends_official() {
    let result = rank("discord", repo_discord(), Ok(aur(include_str!("fixtures/search/aur-discord.json"))), &HashSet::new());

    let recommended = result.recommended.unwrap();
    assert_eq!(recommended.name, "discord");
    assert_eq!(recommended.source, Source::Repo { repo: "extra".into() });
    let others: Vec<&str> = result.others.iter().map(|c| c.name.as_str()).collect();
    assert!(others.contains(&"discord-canary") && others.contains(&"discord-ptb"), "{others:?}");
    assert!(others.len() <= 3);
    assert!(!result.aur_unavailable);
}

#[test]
fn t3_recommends_bin() {
    let result = rank("T3 Code", vec![], Ok(aur(include_str!("fixtures/search/aur-t3.json"))), &HashSet::new());

    assert_eq!(result.recommended.unwrap().name, "t3code-bin");
    assert_eq!(result.others.len(), 3);
    assert_eq!(result.others[0].name, "t3code-nightly-bin");
}

#[test]
fn aur_error_keeps_repo_results() {
    let result = rank("discord", repo_discord(), Err(TermaError::AurUnavailable), &HashSet::new());

    assert_eq!(result.recommended.unwrap().name, "discord");
    assert!(result.aur_unavailable);
}

#[test]
fn filters_non_apps_and_duplicates() {
    let repo = vec![repo_pkg("foo", Some(true), None), repo_pkg("foo-docs", Some(false), None), repo_pkg("foo-qt", None, None)];
    let aur = vec![aur_pkg("foo", 900), aur_pkg("foo-flatpak", 900), aur_pkg("foo-bin", 5)];

    let result = rank("foo", repo, Ok(aur), &HashSet::new());

    let rec = result.recommended.unwrap();
    assert_eq!((rec.name.as_str(), &rec.source), ("foo", &Source::Repo { repo: "extra".into() }));
    let others: Vec<&str> = result.others.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(others, vec!["foo-bin", "foo-qt"]);
}

#[test]
fn unrelated_names_are_dropped() {
    let result = rank("discord", vec![], Ok(vec![aur_pkg("betterdiscordctl", 900)]), &HashSet::new());
    assert!(result.recommended.is_none());
    assert!(result.others.is_empty());
}

#[test]
fn installed_flag() {
    let foreign: HashSet<String> = ["foo-bin".to_string()].into();
    let result = rank("foo", vec![repo_pkg("foo", Some(true), Some("1-1"))], Ok(vec![aur_pkg("foo-bin", 5)]), &foreign);

    assert!(result.recommended.unwrap().installed);
    assert!(result.others[0].installed);
}

#[test]
fn candidate_fields() {
    let result = rank("discord", repo_discord(), Ok(vec![]), &HashSet::new());
    let c = result.recommended.unwrap();
    assert_eq!(c.display_name, "Discord");
    assert_eq!(c.download_size, Some(2_170_552));
    assert_eq!(c.version, "1:1.0.167-1");
    assert_eq!(search::Candidate { ..c.clone() }, c);
}

#[test]
fn only_web_addresses_reach_the_surface() {
    let mut official = repo_pkg("discord", Some(true), None);
    official.url = Some("https://discord.com".into());
    let mut canary = aur_pkg("discord-canary", 10);
    canary.url = Some("http://discord.com".into());
    let mut ptb = aur_pkg("discord-ptb", 5);
    ptb.url = Some("file:///etc/passwd".into());

    let result = rank("discord", vec![official], Ok(vec![canary, ptb]), &HashSet::new());

    assert_eq!(result.recommended.unwrap().url.as_deref(), Some("https://discord.com"));
    let url = |name: &str| result.others.iter().find(|c| c.name == name).unwrap().url.clone();
    assert_eq!(url("discord-canary").as_deref(), Some("http://discord.com"));
    assert_eq!(url("discord-ptb"), None);
}

#[test]
fn web_url_cases() {
    assert!(search::web_url("https://github.com/pingdotgg/t3code"));
    assert!(search::web_url("http://example.org"));
    for bad in ["file:///etc/passwd", "javascript:alert(1)", "https://", "ftp://example.org", "HTTPS://Example.org evil"] {
        assert!(!search::web_url(bad), "{bad}");
    }
}
