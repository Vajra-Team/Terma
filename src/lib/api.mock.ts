// Beispieldaten wie in den Entwürfen, nur für VITE_MOCK=1.

import type {
  Candidate,
  FileInspection,
  FlatpakDuplicate,
  InstallOutcome,
  InstallPlan,
  IntegratedApp,
  Overview,
  Program,
  Progress,
  SearchResult,
  Settings,
  UpdateReport,
} from './api';

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const repo = { kind: 'Repo', repo: 'extra' } as const;
const aur = { kind: 'Aur' } as const;
const appimage = { kind: 'AppImage' } as const;

const program = (name: string, source: Program['source'], update: Program['update'] = 'None'): Program => ({
  name,
  icon: null,
  source,
  update,
  desktop_file: `/usr/share/applications/${name.toLowerCase()}.desktop`,
  package: source.kind === 'AppImage' ? null : name.toLowerCase(),
});

let programs: Program[] = [
  program('Discord', repo),
  program('Firefox', repo),
  program('Sidra', appimage, 'SelfUpdating'),
  program('Spotify', aur, 'Ready'),
  program('Steam', repo),
  program('T3 Code', appimage, 'Ready'),
];
let duplicates: FlatpakDuplicate[] = [{ name: 'Discord', flatpak_id: 'com.discordapp.Discord', installation: 'system' }];
let settings: Settings = { passwordless: false };
let listeners: ((p: Progress) => void)[] = [];

const candidate = (
  name: string,
  display_name: string,
  description: string,
  source: Candidate['source'],
  size: number | null,
  installed = false,
  url: string | null = null,
): Candidate => ({
  name,
  display_name,
  description,
  source,
  version: '1.0-1',
  download_size: size,
  installed,
  url,
  icon: null,
});

export async function overview(): Promise<Overview> {
  await wait(300);
  return { pending_count: 2, programs, flatpak_duplicates: duplicates };
}

export async function search(query: string): Promise<SearchResult> {
  await wait(250);
  const q = query.toLowerCase().replace(/\s/g, '');
  if (q.startsWith('t3')) {
    return {
      recommended: candidate('t3code-bin', 'T3code', 'Desktop control surface for local coding agents', aur, null, false, 'https://github.com/pingdotgg/t3code'),
      others: [candidate('t3code-nightly-bin', 'T3code Nightly', 'Nightly-Version', aur, null, false, 'https://github.com/pingdotgg/t3code'), candidate('t3-code-git', 'T3 Code Git', 'Aus dem Quelltext', aur, null)],
      aur_unavailable: false,
    };
  }
  if (q.startsWith('disc')) {
    return {
      recommended: candidate('discord', 'Discord', 'Chat, Sprache und Video für Gruppen', repo, 98_500_000, true, 'https://discord.com'),
      others: [candidate('discord-canary', 'Discord Canary', 'Testversion mit neuen Funktionen', aur, null, false, 'https://discord.com'), candidate('discord-ptb', 'Discord Ptb', 'Öffentliche Vorabversion', aur, null)],
      aur_unavailable: false,
    };
  }
  return { recommended: null, others: [], aur_unavailable: false };
}

export async function inspectFile(path: string): Promise<FileInspection> {
  await wait(200);
  return {
    path,
    kind: 'AppImage',
    app_name: 'T3 Code',
    app_version: '0.1.1',
    icon: null,
    source_match: candidate('t3code-bin', 'T3code', 'Desktop control surface for local coding agents', aur, null, false, 'https://github.com/pingdotgg/t3code'),
    self_updating: true,
  };
}

export async function planInstall(c: Candidate, fromFile: string | null): Promise<InstallPlan> {
  await wait(300);
  return {
    candidate: c,
    system_update: { package_count: 12, download_bytes: 1_234_567_890, includes_kernel: false },
    aur: c.source.kind === 'Aur' ? { repo_deps: [], build_order: [] } : null,
    warnings: c.source.kind === 'Aur' ? [{ package: c.name, reasons: [{ kind: 'FewVotes', votes: 11 }] }] : [],
    from_file: fromFile,
  };
}

export function onProgress(cb: (p: Progress) => void): Promise<() => void> {
  listeners.push(cb);
  return Promise.resolve(() => (listeners = listeners.filter((l) => l !== cb)));
}

async function play(steps: Progress[]) {
  for (const p of steps) {
    await wait(350);
    listeners.forEach((l) => l(p));
  }
}

export async function install(plan: InstallPlan): Promise<InstallOutcome> {
  await play([
    { kind: 'Step', data: { kind: 'Building', package: plan.candidate.name } },
    { kind: 'Line', data: '==> Making package: t3code-bin 0.1.5-1' },
    { kind: 'Line', data: '==> Finished making: t3code-bin 0.1.5-1' },
    { kind: 'Step', data: { kind: 'Installing', package: plan.candidate.name } },
    { kind: 'Line', data: 'installing t3code-bin...' },
  ]);
  return { restart_recommended: false };
}

export async function setupAppImage(path: string): Promise<IntegratedApp> {
  await wait(300);
  return { name: 'T3 Code', appimage: path, desktop_file: '', icon: null };
}

export async function remove(p: Program): Promise<void> {
  await play([{ kind: 'Step', data: { kind: 'Removing', name: p.name } }]);
  programs = programs.filter((x) => x !== p);
}

export async function removePreview(p: Program): Promise<string[]> {
  await wait(150);
  return p.name === 'Firefox' ? ['firefox-i18n-de'] : [];
}

export async function removeFlatpak(d: FlatpakDuplicate): Promise<void> {
  await wait(300);
  duplicates = duplicates.filter((x) => x !== d);
}

export async function updateAll(): Promise<UpdateReport> {
  await play([{ kind: 'Step', data: { kind: 'UpdatingSystem' } }, { kind: 'Line', data: ':: Starting full system upgrade...' }]);
  return { skipped: [], failed: [], appimages_updated: ['T3 Code'], appimages_failed: [], aur_unreachable: false, restart_recommended: false };
}

export async function forceUpdateAur(_names: string[]): Promise<void> {
  await wait(300);
}

export async function getSettings(): Promise<Settings> {
  return settings;
}

export async function setPasswordless(on: boolean): Promise<void> {
  await wait(200);
  settings = { passwordless: on };
}

export async function startupFile(): Promise<string | null> {
  return new URLSearchParams(location.search).get('datei');
}

export async function openUrl(url: string): Promise<void> {
  window.open(url, '_blank');
}
