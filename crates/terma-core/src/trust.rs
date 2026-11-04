//! Vertrauensregel für AUR-Pakete. Eine leere Liste heißt: ohne Nachfrage installierbar.

use serde::{Deserialize, Serialize};

use crate::aur::{domain_of, source_domains, AurPackage, AurRecord, SrcInfo};

/// Schwellen, von Jamie am 30.10.2026 bestätigt.
pub const MIN_AGE_DAYS: i64 = 180;
pub const MIN_VOTES: u32 = 20;
pub const MIN_POPULARITY: f64 = 0.5;

const DAY: i64 = 86_400;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TrustReason {
    TooNew { days: i64 },
    FewVotes { votes: u32 },
    LowPopularity,
    Orphaned,
    ImitatesName { original: String },
    SourceMismatch { source: String, project: String },
    MaintainerChanged { old: Option<String>, new: Option<String> },
    NewSource { domain: String },
    RiskyDependency { package: String },
}

pub fn evaluate(pkg: &AurPackage, src: &SrcInfo, now: i64) -> Vec<TrustReason> {
    let mut reasons = Vec::new();
    let days = (now - pkg.first_submitted) / DAY;
    if days < MIN_AGE_DAYS {
        reasons.push(TrustReason::TooNew { days });
    }
    if pkg.num_votes < MIN_VOTES {
        reasons.push(TrustReason::FewVotes { votes: pkg.num_votes });
    }
    if pkg.popularity < MIN_POPULARITY {
        reasons.push(TrustReason::LowPopularity);
    }
    if pkg.maintainer.is_none() {
        reasons.push(TrustReason::Orphaned);
    }
    if let Some(original) = imitated_name(&pkg.name) {
        reasons.push(TrustReason::ImitatesName { original });
    }
    reasons.extend(source_mismatches(pkg, src));
    reasons
}

/// Nachprüfung bei Updates: neuer Betreuer, neue Download-Domain, `-bin`-Quelle passt nicht.
pub fn recheck(old: &AurRecord, pkg: &AurPackage, src: &SrcInfo) -> Vec<TrustReason> {
    let mut reasons = Vec::new();
    if old.maintainer != pkg.maintainer {
        reasons.push(TrustReason::MaintainerChanged { old: old.maintainer.clone(), new: pkg.maintainer.clone() });
    }
    for domain in source_domains(src) {
        if !old.source_domains.contains(&domain) {
            reasons.push(TrustReason::NewSource { domain });
        }
    }
    reasons.extend(source_mismatches(pkg, src));
    reasons
}

/// `firefox-patch-bin` → `firefox`. So waren 2025 Schadpakete getarnt.
fn imitated_name(name: &str) -> Option<String> {
    let base = name.strip_suffix("-bin").unwrap_or(name);
    ["-patched", "-patch", "-fix"]
        .iter()
        .find_map(|suffix| base.strip_suffix(suffix))
        .filter(|original| !original.is_empty())
        .map(str::to_string)
}

/// Bei `-bin`-Paketen muss jede Download-Domain zur Projekt-Domain passen.
fn source_mismatches(pkg: &AurPackage, src: &SrcInfo) -> Vec<TrustReason> {
    if !pkg.name.ends_with("-bin") {
        return Vec::new();
    }
    let Some(project) = pkg.url.as_deref().and_then(domain_of) else { return Vec::new() };
    source_domains(src)
        .into_iter()
        .filter(|d| *d != project)
        .map(|source| TrustReason::SourceMismatch { source, project: project.clone() })
        .collect()
}
