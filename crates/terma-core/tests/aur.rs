use std::sync::Arc;

use terma_core::aur::{self, AurClient, AurRecord, AurStateStore};
use terma_core::dirs::UserDirs;
use terma_core::error::TermaError;
use terma_core::http::FakeHttp;

const SEARCH: &str = include_str!("fixtures/aur/search-t3.json");
const INFO: &str = include_str!("fixtures/aur/info-t3code-bin.json");
const SRCINFO: &str = include_str!("fixtures/aur/srcinfo-t3code-bin.txt");
const BASE: &str = "https://aur.archlinux.org";

#[test]
fn parse_rpc_t3code_bin() {
    let pkgs = aur::parse_rpc(INFO).unwrap();
    let p = &pkgs[0];
    assert_eq!(p.name, "t3code-bin");
    assert_eq!(p.package_base, "t3code-bin");
    assert_eq!(p.num_votes, 11);
    assert_eq!(p.maintainer.as_deref(), Some("maria_rcks"));
    assert_eq!(p.url.as_deref(), Some("https://github.com/pingdotgg/t3code"));
    assert_eq!(p.first_submitted, 1772842332);
    assert!(p.depends.contains(&"gtk3".to_string()));
    assert!(p.make_depends.is_empty());
    assert_eq!(p.provides, vec!["t3code=0.1.5".to_string()]);
}

#[test]
fn parse_rpc_error_is_unavailable() {
    let err = aur::parse_rpc(r#"{"version":5,"type":"error","resultcount":0,"results":[],"error":"Too many package results."}"#).unwrap_err();
    assert_eq!(err, TermaError::AurUnavailable);
}

#[test]
fn client_search_and_info_urls() {
    let http = FakeHttp::new()
        .on(&format!("{BASE}/rpc/v5/search/t3code?by=name"), SEARCH)
        .on(&format!("{BASE}/rpc/v5/info?arg%5B%5D=t3code-bin"), INFO)
        .on(&format!("{BASE}/cgit/aur.git/plain/.SRCINFO?h=t3code-bin"), SRCINFO);
    let client = AurClient::new(BASE, Arc::new(http));

    let found: Vec<String> = client.search("t3code").unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(found, vec!["t3code-bin", "t3code-nightly-bin"]);
    assert_eq!(client.info(&["t3code-bin".to_string()]).unwrap()[0].num_votes, 11);
    assert!(client.srcinfo("t3code-bin").unwrap().sources[0].starts_with("https://github.com/pingdotgg/t3code/releases/download/"));
    assert_eq!(client.search("x").unwrap(), vec![], "Das AUR verlangt mindestens zwei Zeichen");
}

#[test]
fn client_network_error_is_unavailable() {
    let client = AurClient::new(BASE, Arc::new(FakeHttp::new()));
    assert_eq!(client.search("discord").unwrap_err(), TermaError::AurUnavailable);
}

#[test]
fn provider_picks_most_voted_and_loads_info() {
    let provides = r#"{"version":5,"type":"search","resultcount":2,"results":[
        {"Name":"libfoo-git","PackageBase":"libfoo-git","Version":"1","NumVotes":2,"Popularity":0.1,"FirstSubmitted":1,"LastModified":1},
        {"Name":"libfoo-bin","PackageBase":"libfoo-bin","Version":"1","NumVotes":40,"Popularity":2.0,"FirstSubmitted":1,"LastModified":1}]}"#;
    let info = r#"{"version":5,"type":"multiinfo","resultcount":1,"results":[
        {"Name":"libfoo-bin","PackageBase":"libfoo-bin","Version":"1","NumVotes":40,"Popularity":2.0,"FirstSubmitted":1,"LastModified":1,"Depends":["glibc"]}]}"#;
    let http = FakeHttp::new()
        .on(&format!("{BASE}/rpc/v5/search/libfoo.so?by=provides"), provides)
        .on(&format!("{BASE}/rpc/v5/info?arg%5B%5D=libfoo-bin"), info);
    let client = AurClient::new(BASE, Arc::new(http));

    let p = client.provider("libfoo.so=1-64").unwrap().unwrap();

    assert_eq!(p.name, "libfoo-bin");
    assert_eq!(p.depends, vec!["glibc".to_string()]);
}

#[test]
fn srcinfo_collects_arch_specific_sources() {
    let src = aur::parse_srcinfo(include_str!("fixtures/aur/srcinfo-mismatch.txt"));
    assert_eq!(src.sources, vec!["https://evil.example.net/app.tar.gz".to_string()]);
    let git = aur::parse_srcinfo("pkgbase = x\n\tsource = x::git+https://gitlab.com/foo/x.git#tag=v1\n\tsource = local.patch\n");
    assert_eq!(aur::source_domains(&git).into_iter().collect::<Vec<_>>(), vec!["gitlab.com/foo".to_string()]);
}

#[test]
fn state_store_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let record = AurRecord { maintainer: Some("maria_rcks".into()), source_domains: ["github.com/pingdotgg".to_string()].into() };

    let mut store = AurStateStore::load(&dirs);
    assert!(store.get("t3code-bin").is_none());
    store.put("t3code-bin", record.clone());
    store.save().unwrap();

    assert_eq!(AurStateStore::load(&dirs).get("t3code-bin"), Some(&record));
}
