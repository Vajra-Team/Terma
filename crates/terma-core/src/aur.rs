//! AUR über die RPC-Schnittstelle (v5) und `.SRCINFO`. Netzfehler heißen hier immer „AUR nicht erreichbar“.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::dirs::UserDirs;
use crate::error::{Result, TermaError};
use crate::http::Http;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AurPackage {
    pub name: String,
    pub package_base: String,
    pub version: String,
    pub description: String,
    pub url: Option<String>,
    pub num_votes: u32,
    pub popularity: f64,
    pub maintainer: Option<String>,
    pub first_submitted: i64,
    pub last_modified: i64,
    pub depends: Vec<String>,
    pub make_depends: Vec<String>,
    pub provides: Vec<String>,
}

/// So liefert die RPC-Schnittstelle ein Paket.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RpcPackage {
    name: String,
    package_base: String,
    version: String,
    description: Option<String>,
    #[serde(rename = "URL")]
    url: Option<String>,
    num_votes: u32,
    popularity: f64,
    maintainer: Option<String>,
    first_submitted: i64,
    last_modified: i64,
    depends: Option<Vec<String>>,
    make_depends: Option<Vec<String>>,
    provides: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RpcResponse {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    results: Vec<RpcPackage>,
}

pub fn parse_rpc(json: &str) -> Result<Vec<AurPackage>> {
    let response: RpcResponse = serde_json::from_str(json).map_err(|_| TermaError::AurUnavailable)?;
    if response.kind == "error" {
        return Err(TermaError::AurUnavailable);
    }
    Ok(response
        .results
        .into_iter()
        .map(|p| AurPackage {
            name: p.name,
            package_base: p.package_base,
            version: p.version,
            description: p.description.unwrap_or_default(),
            url: p.url.filter(|u| !u.is_empty()),
            num_votes: p.num_votes,
            popularity: p.popularity,
            maintainer: p.maintainer,
            first_submitted: p.first_submitted,
            last_modified: p.last_modified,
            depends: p.depends.unwrap_or_default(),
            make_depends: p.make_depends.unwrap_or_default(),
            provides: p.provides.unwrap_or_default(),
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SrcInfo {
    pub sources: Vec<String>,
}

/// `source` und `source_<arch>`, ohne den Dateinamen-Präfix `name::`.
pub fn parse_srcinfo(text: &str) -> SrcInfo {
    let sources = text
        .lines()
        .filter_map(|l| l.trim().split_once(" = "))
        .filter(|(k, _)| *k == "source" || k.starts_with("source_"))
        .map(|(_, v)| v.rsplit("::").next().unwrap_or(v).trim().to_string())
        .collect();
    SrcInfo { sources }
}

/// Domains aller Quellen aus dem Netz. Lokale Dateien zählen nicht.
pub fn source_domains(src: &SrcInfo) -> BTreeSet<String> {
    src.sources.iter().filter_map(|s| domain_of(s)).collect()
}

/// `https://www.github.com/Foo/app` → `github.com/foo`, `https://dl.example.net/x` → `example.net`.
/// Bei GitHub, GitLab und Codeberg zählt der Besitzer mit; `raw.githubusercontent.com` gilt als GitHub.
pub fn domain_of(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    let rest = rest.split(['#', '?']).next().unwrap_or(rest);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.rsplit('@').next().unwrap_or(host).split(':').next().unwrap_or(host).to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    let owner = path.split('/').next().filter(|o| !o.is_empty()).map(str::to_lowercase);
    let forge = match host.as_str() {
        "github.com" | "raw.githubusercontent.com" | "codeload.github.com" => Some("github.com"),
        "gitlab.com" => Some("gitlab.com"),
        "codeberg.org" => Some("codeberg.org"),
        _ => None,
    };
    if let Some(forge) = forge {
        return Some(match owner {
            Some(owner) => format!("{forge}/{owner}"),
            None => forge.to_string(),
        });
    }
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    if labels.is_empty() {
        return None;
    }
    Some(labels[labels.len().saturating_sub(2)..].join("."))
}

pub struct AurClient {
    base: String,
    http: Arc<dyn Http>,
}

/// Prozent-Kodierung für Pfad- und Query-Teile.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl AurClient {
    pub fn new(base_url: &str, http: Arc<dyn Http>) -> AurClient {
        AurClient { base: base_url.trim_end_matches('/').to_string(), http }
    }

    fn rpc(&self, url: &str) -> Result<Vec<AurPackage>> {
        let body = self.http.get_string(url).map_err(|_| TermaError::AurUnavailable)?;
        parse_rpc(&body)
    }

    /// Suche im Paketnamen. Das AUR verlangt mindestens zwei Zeichen.
    pub fn search(&self, term: &str) -> Result<Vec<AurPackage>> {
        if term.trim().chars().count() < 2 {
            return Ok(Vec::new());
        }
        self.rpc(&format!("{}/rpc/v5/search/{}?by=name", self.base, encode(term.trim())))
    }

    pub fn info(&self, names: &[String]) -> Result<Vec<AurPackage>> {
        let mut all = Vec::new();
        for chunk in names.chunks(100) {
            let args: Vec<String> = chunk.iter().map(|n| format!("arg%5B%5D={}", encode(n))).collect();
            all.extend(self.rpc(&format!("{}/rpc/v5/info?{}", self.base, args.join("&")))?);
        }
        Ok(all)
    }

    /// Das AUR-Paket mit den meisten Stimmen, das `dep` bereitstellt, mit vollständigen Abhängigkeiten.
    pub fn provider(&self, dep: &str) -> Result<Option<AurPackage>> {
        let name = dep.split(['<', '>', '=']).next().unwrap_or(dep);
        let best = self
            .rpc(&format!("{}/rpc/v5/search/{}?by=provides", self.base, encode(name)))?
            .into_iter()
            .max_by_key(|p| p.num_votes);
        match best {
            Some(p) => Ok(self.info(&[p.name])?.into_iter().next()),
            None => Ok(None),
        }
    }

    pub fn srcinfo(&self, pkgbase: &str) -> Result<SrcInfo> {
        let url = format!("{}/cgit/aur.git/plain/.SRCINFO?h={}", self.base, encode(pkgbase));
        let text = self.http.get_string(&url).map_err(|_| TermaError::AurUnavailable)?;
        Ok(parse_srcinfo(&text))
    }
}

/// Was Terma sich bei der Installation über ein AUR-Paket merkt, um Übernahmen zu erkennen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AurRecord {
    pub maintainer: Option<String>,
    pub source_domains: BTreeSet<String>,
}

pub struct AurStateStore {
    path: PathBuf,
    records: HashMap<String, AurRecord>,
}

impl AurStateStore {
    pub fn load(dirs: &UserDirs) -> AurStateStore {
        let path = dirs.data.join("aur-state.json");
        let records = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        AurStateStore { path, records }
    }

    pub fn get(&self, name: &str) -> Option<&AurRecord> {
        self.records.get(name)
    }

    pub fn put(&mut self, name: &str, record: AurRecord) {
        self.records.insert(name.to_string(), record);
    }

    pub fn save(&self) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&self.records).map_err(|e| TermaError::Io(e.to_string()))?;
        std::fs::write(&self.path, text)?;
        Ok(())
    }
}
