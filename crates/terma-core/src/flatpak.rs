//! Flatpak nur, um doppelt installierte Programme zu finden und die Flatpak-Version zu entfernen.

use serde::{Deserialize, Serialize};

use crate::error::{Result, TermaError};
use crate::programs::Program;
use crate::runner::{LineTail, Runner};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Installation {
    User,
    System,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlatpakApp {
    pub id: String,
    pub name: String,
    pub installation: Installation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlatpakDuplicate {
    pub name: String,
    pub flatpak_id: String,
    pub installation: Installation,
}

/// Leer, wenn Flatpak nicht installiert ist.
pub fn list_apps(runner: &dyn Runner) -> Vec<FlatpakApp> {
    let Ok((0, out)) = runner.output(&["flatpak", "list", "--app", "--columns=application,name,installation"]) else { return Vec::new() };
    out.lines()
        .filter_map(|l| {
            let mut cols = l.split('\t');
            let (id, name, installation) = (cols.next()?, cols.next()?, cols.next()?);
            let installation = if installation.trim() == "user" { Installation::User } else { Installation::System };
            Some(FlatpakApp { id: id.trim().into(), name: name.trim().into(), installation })
        })
        .collect()
}

pub fn duplicates(programs: &[Program], apps: &[FlatpakApp]) -> Vec<FlatpakDuplicate> {
    apps.iter()
        .filter_map(|app| {
            let program = programs.iter().find(|p| p.name.eq_ignore_ascii_case(&app.name))?;
            Some(FlatpakDuplicate { name: program.name.clone(), flatpak_id: app.id.clone(), installation: app.installation.clone() })
        })
        .collect()
}

pub fn uninstall(runner: &dyn Runner, d: &FlatpakDuplicate) -> Result<()> {
    let scope = match d.installation {
        Installation::User => "--user",
        Installation::System => "--system",
    };
    let mut tail = LineTail::default();
    let code = runner.run(&["flatpak", "uninstall", "--noninteractive", "-y", scope, &d.flatpak_id], &[], None, &mut |l| tail.push(l))?;
    if code != 0 {
        return Err(TermaError::Command { code, tail: tail.text() });
    }
    Ok(())
}
