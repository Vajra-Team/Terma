//! Suche über offizielle Quellen und AUR: eine Empfehlung, darunter höchstens drei Varianten.
//!
//! Rangfolge in Klassen statt Punkten: zuerst dasselbe Programm (exakter Name oder eine Variante wie
//! `-canary`, `-bin`), dann ähnlich benannte Programme. Innerhalb der Klasse: offizielle Quelle,
//! stabile vor Test-Versionen, `-bin` vor Bauen aus dem Quelltext, dann Stimmen.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::aur::AurPackage;
use crate::error::Result;
use crate::pacman::RepoPackage;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Source {
    Repo { repo: String },
    Aur,
    AppImage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub source: Source,
    pub version: String,
    pub download_size: Option<u64>,
    pub installed: bool,
    /// Projektseite, nur http(s). Beim AUR trägt sie der Paketierer ein, niemand prüft sie.
    pub url: Option<String>,
    /// Icon aus den Arch-AppStream-Daten, nur für offizielle Pakete.
    pub icon: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub recommended: Option<Candidate>,
    pub others: Vec<Candidate>,
    pub aur_unavailable: bool,
}

/// Verpackung, die am Programm nichts ändert.
const PACKAGING: &[&str] = &["bin", "appimage"];
/// Test- und Entwicklerversionen desselben Programms.
const UNSTABLE: &[&str] =
    &["git", "nightly", "canary", "ptb", "beta", "alpha", "dev", "development", "docker", "unstable", "insiders", "preview", "latest"];

const MAX_OTHERS: usize = 3;

/// Nur Adressen, die der Browser öffnen soll: `http(s)://` mit Host, ohne Leerzeichen.
pub fn web_url(url: &str) -> bool {
    ["https://", "http://"]
        .iter()
        .find_map(|scheme| url.strip_prefix(scheme))
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/') && !url.contains(char::is_whitespace))
}

/// `"T3 Code"` → `["t3code", "t3-code"]`
pub fn query_terms(q: &str) -> Vec<String> {
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    match words.len() {
        0 => Vec::new(),
        1 => words,
        _ => vec![words.concat(), words.join("-")],
    }
}

/// `"discord-canary"` → `"Discord Canary"`, `"t3code-bin"` → `"T3code"`
pub fn display_name(pkg: &str) -> String {
    pkg.strip_suffix("-bin")
        .unwrap_or(pkg)
        .split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map(|c| c.to_uppercase().chain(chars).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `"T3 Code (Beta)"` → `"T3 Code "`
pub fn without_parentheses(name: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in name.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Vergleichsform: klein, ohne Klammerzusatz, ohne `-bin`/`-git`/`-appimage`, ohne Leerzeichen und Bindestriche.
pub fn normalize(name: &str) -> String {
    let mut s = without_parentheses(name).trim().to_lowercase();
    for suffix in ["-bin", "-git", "-appimage"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.to_string();
        }
    }
    s.chars().filter(|c| !c.is_whitespace() && *c != '-').collect()
}

fn tokens(name: &str) -> Vec<String> {
    without_parentheses(name)
        .to_lowercase()
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Name ohne Verpackungs- und Versionszusätze: `discord-canary` → `discord`.
fn core(name: &str) -> String {
    tokens(name).into_iter().filter(|t| !PACKAGING.contains(&t.as_str()) && !UNSTABLE.contains(&t.as_str())).collect()
}

/// 2 = dasselbe Programm, 1 = ähnlich benannt, 0 = gehört nicht dazu.
fn relation(name: &str, query: &str) -> u8 {
    if normalize(name) == query || core(name) == query {
        2
    } else if normalize(name).starts_with(query) {
        1
    } else {
        0
    }
}

pub fn rank(query: &str, repo: Vec<RepoPackage>, aur: Result<Vec<AurPackage>>, foreign_installed: &HashSet<String>) -> SearchResult {
    let q = normalize(query);
    let aur_unavailable = aur.is_err();
    let repo_names: HashSet<String> = repo.iter().map(|p| p.name.clone()).collect();

    // (Kandidat, Stimmen)
    let mut found: Vec<(Candidate, u32)> = Vec::new();
    for p in repo.into_iter().filter(|p| p.has_desktop_entry != Some(false)) {
        let candidate = Candidate {
            display_name: display_name(&p.name),
            installed: p.installed_version.is_some(),
            source: Source::Repo { repo: p.repo },
            download_size: Some(p.download_size),
            url: p.url.filter(|u| web_url(u)),
            icon: None,
            name: p.name,
            description: p.description,
            version: p.version,
        };
        found.push((candidate, 0));
    }
    for p in aur.unwrap_or_default().into_iter().filter(|p| !repo_names.contains(&p.name) && !p.name.contains("flatpak")) {
        let candidate = Candidate {
            display_name: display_name(&p.name),
            installed: foreign_installed.contains(&p.name),
            source: Source::Aur,
            download_size: None,
            url: p.url.filter(|u| web_url(u)),
            icon: None,
            name: p.name,
            description: p.description,
            version: p.version,
        };
        found.push((candidate, p.num_votes));
    }

    found.retain(|(c, _)| relation(&c.name, &q) > 0);
    found.sort_by_key(|(c, votes)| {
        let unstable = tokens(&c.name).iter().any(|t| UNSTABLE.contains(&t.as_str()));
        let official = matches!(c.source, Source::Repo { .. });
        (Reverse(relation(&c.name, &q)), Reverse(official), unstable, Reverse(c.name.ends_with("-bin")), Reverse(*votes), c.name.clone())
    });

    let mut ranked = found.into_iter().map(|(c, _)| c);
    SearchResult { recommended: ranked.next(), others: ranked.take(MAX_OTHERS).collect(), aur_unavailable }
}
