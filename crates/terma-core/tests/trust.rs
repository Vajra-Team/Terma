use std::collections::BTreeSet;

use terma_core::aur::{self, AurPackage, AurRecord, SrcInfo};
use terma_core::trust::{evaluate, recheck, TrustReason};

/// 30.10.2026, 00:00 UTC
const NOW: i64 = 1_793_318_400;
const DAY: i64 = 86_400;

fn pkg(name: &str, url: &str, votes: u32, popularity: f64, maintainer: Option<&str>, age_days: i64) -> AurPackage {
    AurPackage {
        name: name.into(),
        package_base: name.into(),
        version: "1.0-1".into(),
        description: String::new(),
        url: Some(url.into()),
        num_votes: votes,
        popularity,
        maintainer: maintainer.map(Into::into),
        first_submitted: NOW - age_days * DAY,
        last_modified: NOW,
        depends: vec![],
        make_depends: vec![],
        provides: vec![],
    }
}

fn src(urls: &[&str]) -> SrcInfo {
    SrcInfo { sources: urls.iter().map(|s| s.to_string()).collect() }
}

#[test]
fn t3code_bin_gets_few_votes_warning() {
    let p = aur::parse_rpc(include_str!("fixtures/aur/info-t3code-bin.json")).unwrap().remove(0);
    let s = aur::parse_srcinfo(include_str!("fixtures/aur/srcinfo-t3code-bin.txt"));

    let reasons = evaluate(&p, &s, NOW);

    assert!(reasons.contains(&TrustReason::FewVotes { votes: 11 }), "{reasons:?}");
    assert!(!reasons.iter().any(|r| matches!(r, TrustReason::SourceMismatch { .. })), "{reasons:?}");
}

#[test]
fn imitating_names_always_warn() {
    for (name, original) in [("firefox-patch-bin", "firefox"), ("librewolf-fix-bin", "librewolf"), ("zen-browser-patched-bin", "zen-browser")] {
        let p = pkg(name, "https://example.org", 500, 10.0, Some("x"), 1000);
        let reasons = evaluate(&p, &src(&["https://example.org/a.tar.gz"]), NOW);
        assert!(reasons.contains(&TrustReason::ImitatesName { original: original.into() }), "{name}: {reasons:?}");
    }
}

#[test]
fn bin_source_mismatch() {
    let p = pkg("app-bin", "https://github.com/foo/app", 500, 10.0, Some("x"), 1000);
    let s = aur::parse_srcinfo(include_str!("fixtures/aur/srcinfo-mismatch.txt"));

    assert_eq!(evaluate(&p, &s, NOW), vec![TrustReason::SourceMismatch { source: "example.net".into(), project: "github.com/foo".into() }]);

    let other_owner = evaluate(&p, &src(&["https://github.com/bar/app/releases/download/v1/app"]), NOW);
    assert_eq!(other_owner, vec![TrustReason::SourceMismatch { source: "github.com/bar".into(), project: "github.com/foo".into() }]);
}

#[test]
fn established_package_is_trusted() {
    let p = pkg("app-bin", "https://www.example.org/app", 300, 5.0, Some("x"), 400);
    assert_eq!(evaluate(&p, &src(&["https://downloads.example.org/app-1.0.tar.gz"]), NOW), vec![]);
}

#[test]
fn orphaned_and_new() {
    let p = pkg("app", "https://example.org", 300, 5.0, None, 10);
    let reasons = evaluate(&p, &src(&[]), NOW);
    assert!(reasons.contains(&TrustReason::Orphaned));
    assert!(reasons.contains(&TrustReason::TooNew { days: 10 }));
}

#[test]
fn recheck_detects_takeover() {
    let old = AurRecord { maintainer: Some("alice".into()), source_domains: BTreeSet::from(["example.org".to_string()]) };
    let p = pkg("app", "https://example.org", 300, 5.0, Some("mallory"), 400);

    let reasons = recheck(&old, &p, &src(&["https://example.org/a.tar.gz", "https://evil.net/b.tar.gz"]));

    assert_eq!(
        reasons,
        vec![
            TrustReason::MaintainerChanged { old: Some("alice".into()), new: Some("mallory".into()) },
            TrustReason::NewSource { domain: "evil.net".into() },
        ]
    );
}
