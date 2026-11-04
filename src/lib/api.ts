// Brücke zum Rust-Kern. Die Typen spiegeln die Structs aus terma-core.
// Mit VITE_MOCK=1 kommen Beispieldaten aus api.mock.ts (für Sichtprüfung im Browser).

import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import * as mock from './api.mock';

export type Source = { kind: 'Repo'; repo: string } | { kind: 'Aur' } | { kind: 'AppImage' };

export interface Candidate {
  name: string;
  display_name: string;
  description: string;
  source: Source;
  version: string;
  download_size: number | null;
  installed: boolean;
  /** Projektseite, nur http(s). Beim AUR vom Paketierer eingetragen. */
  url: string | null;
  icon: string | null;
}

export interface SearchResult {
  recommended: Candidate | null;
  others: Candidate[];
  aur_unavailable: boolean;
}

export type UpdateState = 'None' | 'Ready' | 'SelfUpdating' | 'NotPossible';

export interface Program {
  name: string;
  icon: string | null;
  source: Source;
  update: UpdateState;
  desktop_file: string;
  package: string | null;
}

export interface FlatpakDuplicate {
  name: string;
  flatpak_id: string;
  installation: 'user' | 'system';
}

export interface Overview {
  pending_count: number;
  programs: Program[];
  flatpak_duplicates: FlatpakDuplicate[];
}

export type TrustReason =
  | { kind: 'TooNew'; days: number }
  | { kind: 'FewVotes'; votes: number }
  | { kind: 'LowPopularity' }
  | { kind: 'Orphaned' }
  | { kind: 'ImitatesName'; original: string }
  | { kind: 'SourceMismatch'; source: string; project: string }
  | { kind: 'MaintainerChanged'; old: string | null; new: string | null }
  | { kind: 'NewSource'; domain: string }
  | { kind: 'RiskyDependency'; package: string };

export interface PackageWarning {
  package: string;
  reasons: TrustReason[];
}

export interface SystemUpdate {
  package_count: number;
  download_bytes: number;
  includes_kernel: boolean;
}

export interface InstallPlan {
  candidate: Candidate;
  system_update: SystemUpdate | null;
  /** Bauplan aus dem Kern, wird unverändert zurückgegeben. */
  aur: { repo_deps: string[]; build_order: unknown[] } | null;
  warnings: PackageWarning[];
  from_file: string | null;
}

export interface InstallOutcome {
  restart_recommended: boolean;
}

export type FileKind = 'AppImage' | 'Deb' | 'Rpm' | 'Tarball' | 'Unknown';

export interface FileInspection {
  path: string;
  kind: FileKind;
  app_name: string;
  app_version: string | null;
  icon: string | null;
  source_match: Candidate | null;
  self_updating: boolean;
}

export interface SkippedUpdate {
  package: string;
  reasons: TrustReason[];
}

export interface FailedUpdate {
  package: string;
  kind: BuildFailure;
}

export interface UpdateReport {
  skipped: SkippedUpdate[];
  failed: FailedUpdate[];
  appimages_updated: string[];
  appimages_failed: string[];
  aur_unreachable: boolean;
  restart_recommended: boolean;
}

export interface IntegratedApp {
  name: string;
  appimage: string;
  desktop_file: string;
  icon: string | null;
}

export interface Settings {
  passwordless: boolean;
}

export type Step =
  | { kind: 'Refreshing' }
  | { kind: 'UpdatingSystem' }
  | { kind: 'Building'; package: string }
  | { kind: 'Installing'; package: string }
  | { kind: 'SettingUpAppImage'; name: string }
  | { kind: 'Removing'; name: string };

export type Progress = { kind: 'Step'; data: Step } | { kind: 'Line'; data: string };

export type BuildFailure = 'PgpKeyUnknown' | 'ChecksumMismatch' | 'SourcesChanged' | 'Other';

export type TermaError =
  | { kind: 'DbLocked' }
  | { kind: 'Cancelled' }
  | { kind: 'Network'; data: string }
  | { kind: 'AurUnavailable' }
  | { kind: 'NotAnAppImage' }
  | { kind: 'UnsupportedFile' }
  | { kind: 'Protected'; data: { package: string } }
  | { kind: 'BuildFailed'; data: { package: string; kind: BuildFailure; log: string } }
  | { kind: 'DependencyCycle'; data: string }
  | { kind: 'UnresolvableDependency'; data: string }
  | { kind: 'Command'; data: { code: number; tail: string } }
  | { kind: 'Io'; data: string };

const MOCK = import.meta.env.VITE_MOCK === '1';

export const overview = (): Promise<Overview> => (MOCK ? mock.overview() : invoke('overview'));
export const search = (query: string): Promise<SearchResult> => (MOCK ? mock.search(query) : invoke('search', { query }));
export const inspectFile = (path: string): Promise<FileInspection> => (MOCK ? mock.inspectFile(path) : invoke('inspect_file', { path }));
export const planInstall = (candidate: Candidate, fromFile: string | null = null): Promise<InstallPlan> =>
  MOCK ? mock.planInstall(candidate, fromFile) : invoke('plan_install', { candidate, fromFile });
export const install = (plan: InstallPlan): Promise<InstallOutcome> => (MOCK ? mock.install(plan) : invoke('install', { plan }));
export const setupAppImage = (path: string): Promise<IntegratedApp> => (MOCK ? mock.setupAppImage(path) : invoke('setup_appimage', { path }));
export const remove = (program: Program): Promise<void> => (MOCK ? mock.remove(program) : invoke('remove', { program }));
export const removePreview = (program: Program): Promise<string[]> => (MOCK ? mock.removePreview(program) : invoke('remove_preview', { program }));
export const removeFlatpak = (dup: FlatpakDuplicate): Promise<void> => (MOCK ? mock.removeFlatpak(dup) : invoke('remove_flatpak', { dup }));
export const updateAll = (): Promise<UpdateReport> => (MOCK ? mock.updateAll() : invoke('update_all'));
export const forceUpdateAur = (names: string[]): Promise<void> => (MOCK ? mock.forceUpdateAur(names) : invoke('force_update_aur', { names }));
export const getSettings = (): Promise<Settings> => (MOCK ? mock.getSettings() : invoke('get_settings'));
export const setPasswordless = (on: boolean): Promise<void> => (MOCK ? mock.setPasswordless(on) : invoke('set_passwordless', { on }));
export const startupFile = (): Promise<string | null> => (MOCK ? mock.startupFile() : invoke('startup_file'));
export const openUrl = (url: string): Promise<void> => (MOCK ? mock.openUrl(url) : invoke('open_url', { url }));

export function onProgress(cb: (p: Progress) => void): Promise<() => void> {
  return MOCK ? mock.onProgress(cb) : listen<Progress>('progress', (e) => cb(e.payload));
}

export function onOpenFile(cb: (path: string) => void): Promise<() => void> {
  return MOCK ? Promise.resolve(() => {}) : listen<string>('open-file', (e) => cb(e.payload));
}

/** Datei ins Fenster gezogen: erster Pfad. */
export function onFileDrop(cb: (path: string) => void): Promise<() => void> {
  if (MOCK) return Promise.resolve(() => {});
  return getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type === 'drop' && e.payload.paths.length > 0) cb(e.payload.paths[0]);
  });
}

/** Lokaler Icon-Pfad → URL für das Webview. Im Browser-Mock gibt es keine Icons. */
export function iconSrc(path: string | null): string | null {
  if (!path || MOCK) return null;
  return convertFileSrc(path);
}
