import { describe, expect, it } from 'vitest';
import type { InstallPlan } from './api';
import { fehlerDetails, fehlerText, nextAfterPlan } from './flow';
import { T } from './texte';

const plan = (over: Partial<InstallPlan> = {}): InstallPlan => ({
  candidate: { name: 'discord', display_name: 'Discord', description: '', source: { kind: 'Repo', repo: 'extra' }, version: '1', download_size: 1, installed: false, url: null, icon: null },
  system_update: null,
  aur: null,
  warnings: [],
  from_file: null,
  ...over,
});

describe('nextAfterPlan', () => {
  it('installiert direkt ohne Warnung und ohne Systemupdate', () => {
    expect(nextAfterPlan(plan())).toBe('install');
  });
  it('fragt nach bei Warnung oder Systemupdate', () => {
    expect(nextAfterPlan(plan({ warnings: [{ package: 't3code-bin', reasons: [{ kind: 'FewVotes', votes: 11 }] }] }))).toBe('confirm');
    expect(nextAfterPlan(plan({ system_update: { package_count: 3, download_bytes: 1, includes_kernel: false } }))).toBe('confirm');
  });
});

describe('fehlerText', () => {
  it('Abbruch im Passwortfenster bleibt still', () => {
    expect(fehlerText({ kind: 'Cancelled' })).toBeNull();
  });
  it('übersetzt bekannte Fehler', () => {
    expect(fehlerText({ kind: 'DbLocked' })).toBe(T.fehler.dbGesperrt);
    expect(fehlerText({ kind: 'BuildFailed', data: { package: 't3code-bin', kind: 'PgpKeyUnknown', log: '/l.log' } })).toBe(T.fehler.bauPgp('t3code-bin'));
    expect(fehlerText({ kind: 'Protected', data: { package: 'dolphin' } })).toBe(T.fehler.geschuetzt('dolphin'));
    expect(fehlerText({ kind: 'NotAnAppImage' })).toBe(T.datei.keinAppImage);
    expect(fehlerText('etwas Unerwartetes')).toBe(T.fehler.allgemein);
  });
  it('Details für „Details zeigen“', () => {
    expect(fehlerDetails({ kind: 'Command', data: { code: 1, tail: 'error: failed' } })).toBe('error: failed');
    expect(fehlerDetails({ kind: 'BuildFailed', data: { package: 'x', kind: 'Other', log: '/home/j/.local/share/terma/logs/x-1.log' } })).toContain('x-1.log');
    expect(fehlerDetails({ kind: 'DbLocked' })).toBeNull();
  });
});

import type { FileInspection } from './api';
import { dateiEmpfehlung } from './flow';

const datei = (over: Partial<FileInspection> = {}): FileInspection => ({
  path: '/mnt/daten/T3.AppImage',
  kind: 'AppImage',
  app_name: 'T3 Code',
  app_version: null,
  icon: null,
  source_match: { name: 't3code-bin', display_name: 'T3code', description: '', source: { kind: 'Aur' }, version: '1', download_size: null, installed: false, url: null, icon: null },
  self_updating: false,
  ...over,
});

describe('dateiEmpfehlung', () => {
  it('AppImage mit eigenem Updater bleibt AppImage, auch wenn es eine Quelle gibt', () => {
    expect(dateiEmpfehlung(datei({ self_updating: true }))).toBe('appimage');
  });
  it('sonst kommt die Quelle zuerst', () => {
    expect(dateiEmpfehlung(datei())).toBe('quelle');
    expect(dateiEmpfehlung(datei({ kind: 'Deb' }))).toBe('quelle');
  });
  it('ohne Quelle: AppImage einrichten oder nichts möglich', () => {
    expect(dateiEmpfehlung(datei({ source_match: null }))).toBe('appimage');
    expect(dateiEmpfehlung(datei({ kind: 'Deb', source_match: null }))).toBe('nichts');
  });
});
