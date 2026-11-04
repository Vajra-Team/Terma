//! Tauri-Befehle: dünne Hülle um `terma_core::ops::Terma`. Alles läuft im Blocking-Pool,
//! damit das Fenster bei pacman, makepkg und Downloads bedienbar bleibt.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use terma_core::error::TermaError;
use terma_core::file_flow::FileInspection;
use terma_core::flatpak::FlatpakDuplicate;
use terma_core::integrate::IntegratedApp;
use terma_core::ops::{InstallOutcome, InstallPlan, Overview, Progress, Settings, Terma, UpdateReport};
use terma_core::programs::Program;
use terma_core::search::{web_url, Candidate, SearchResult};

pub struct AppState {
    pub terma: Arc<Terma>,
    pub startup_file: Option<String>,
}

type Res<T> = Result<T, TermaError>;

/// Das erste Argument, das eine vorhandene Datei ist (von Rechtsklick, Doppelklick oder zweiter Instanz).
pub fn file_arg(args: &[String]) -> Option<String> {
    args.iter()
        .filter(|a| !a.starts_with('-'))
        .map(Path::new)
        .find(|p| p.is_file())
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().into_owned())
}

async fn blocking<T: Send + 'static>(state: &State<'_, AppState>, f: impl FnOnce(&Terma) -> Res<T> + Send + 'static) -> Res<T> {
    let terma = state.terma.clone();
    tauri::async_runtime::spawn_blocking(move || f(&terma)).await.map_err(|e| TermaError::Io(e.to_string()))?
}

fn emitter(app: AppHandle) -> impl FnMut(Progress) {
    move |p| {
        let _ = app.emit("progress", p);
    }
}

#[tauri::command]
pub async fn overview(state: State<'_, AppState>) -> Res<Overview> {
    blocking(&state, |t| t.overview()).await
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> Res<SearchResult> {
    blocking(&state, move |t| t.search(&query)).await
}

#[tauri::command]
pub async fn inspect_file(state: State<'_, AppState>, path: String) -> Res<FileInspection> {
    blocking(&state, move |t| t.inspect_file(Path::new(&path))).await
}

#[tauri::command]
pub async fn plan_install(state: State<'_, AppState>, candidate: Candidate, from_file: Option<String>) -> Res<InstallPlan> {
    blocking(&state, move |t| t.plan_install(&candidate, from_file.as_deref().map(Path::new))).await
}

#[tauri::command]
pub async fn install(app: AppHandle, state: State<'_, AppState>, plan: InstallPlan) -> Res<InstallOutcome> {
    blocking(&state, move |t| t.install(&plan, &mut emitter(app))).await
}

#[tauri::command]
pub async fn setup_appimage(state: State<'_, AppState>, path: String) -> Res<IntegratedApp> {
    blocking(&state, move |t| t.setup_appimage(&PathBuf::from(path))).await
}

#[tauri::command]
pub async fn remove(app: AppHandle, state: State<'_, AppState>, program: Program) -> Res<()> {
    blocking(&state, move |t| t.remove(&program, &mut emitter(app))).await
}

#[tauri::command]
pub async fn remove_preview(state: State<'_, AppState>, program: Program) -> Res<Vec<String>> {
    blocking(&state, move |t| t.remove_preview(&program)).await
}

#[tauri::command]
pub async fn remove_flatpak(state: State<'_, AppState>, dup: FlatpakDuplicate) -> Res<()> {
    blocking(&state, move |t| t.remove_flatpak(&dup)).await
}

#[tauri::command]
pub async fn update_all(app: AppHandle, state: State<'_, AppState>) -> Res<UpdateReport> {
    blocking(&state, move |t| t.update_all(&mut emitter(app))).await
}

#[tauri::command]
pub async fn force_update_aur(app: AppHandle, state: State<'_, AppState>, names: Vec<String>) -> Res<()> {
    blocking(&state, move |t| t.force_update_aur(&names, &mut emitter(app))).await
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Res<Settings> {
    blocking(&state, |t| Ok(t.settings())).await
}

#[tauri::command]
pub async fn set_passwordless(state: State<'_, AppState>, on: bool) -> Res<()> {
    blocking(&state, move |t| t.set_passwordless(on)).await
}

/// Öffnet eine Projektseite im Browser. Nur http(s): beim AUR stammt die Adresse vom Paketierer.
#[tauri::command]
pub fn open_url(url: String) -> Res<()> {
    if !web_url(&url) {
        return Err(TermaError::Io(format!("keine Web-Adresse: {url}")));
    }
    let mut child = std::process::Command::new("xdg-open").arg(&url).spawn().map_err(|e| TermaError::Io(e.to_string()))?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[tauri::command]
pub fn startup_file(state: State<'_, AppState>) -> Option<String> {
    state.startup_file.clone()
}
