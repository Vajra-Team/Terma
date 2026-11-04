// Ablaufentscheidungen der Oberfläche, ohne Svelte testbar.

import type { FileInspection, InstallPlan, TermaError } from './api';
import { T, bauFehler } from './texte';

/** Mit Warnung oder Systemupdate fragt Terma vorher nach, sonst geht es direkt los. */
export function nextAfterPlan(plan: InstallPlan): 'confirm' | 'install' {
  return plan.warnings.length > 0 || plan.system_update !== null ? 'confirm' : 'install';
}

function asError(e: unknown): TermaError | null {
  return e && typeof e === 'object' && 'kind' in e ? (e as TermaError) : null;
}

/** Satz für die Oberfläche. `null` heißt: still zurück (Passwortfenster abgebrochen). */
export function fehlerText(e: unknown): string | null {
  const err = asError(e);
  switch (err?.kind) {
    case 'Cancelled':
      return null;
    case 'DbLocked':
      return T.fehler.dbGesperrt;
    case 'Network':
      return T.fehler.netz;
    case 'AurUnavailable':
      return T.suche.aurNichtErreichbar;
    case 'NotAnAppImage':
      return T.datei.keinAppImage;
    case 'Protected':
      return T.fehler.geschuetzt(err.data.package);
    case 'BuildFailed':
      return bauFehler(err.data.kind, err.data.package);
    default:
      return T.fehler.allgemein;
  }
}

/** Technischer Hintergrund für „Details zeigen“, falls es einen gibt. */
export function fehlerDetails(e: unknown): string | null {
  const err = asError(e);
  switch (err?.kind) {
    case 'Command':
      return err.data.tail;
    case 'BuildFailed':
      return `Log: ${err.data.log}`;
    case 'Network':
    case 'Io':
    case 'DependencyCycle':
    case 'UnresolvableDependency':
      return err.data;
    default:
      return null;
  }
}

/** Was das Datei-Fenster empfiehlt. AppImages mit eigenem Updater bleiben AppImages (Jamie, 30.10.2026),
 *  damit der Update-Knopf im Programm weiter funktioniert. */
export function dateiEmpfehlung(i: FileInspection): 'appimage' | 'quelle' | 'nichts' {
  if (i.kind === 'AppImage' && (i.self_updating || !i.source_match)) return 'appimage';
  if (i.source_match) return 'quelle';
  return 'nichts';
}
