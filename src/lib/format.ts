// Zahlen für die Oberfläche: Dezimaltrenner der Sprache, Einheiten zur Basis 1000.

import { T } from './texte';

const UNITS = ['B', 'kB', 'MB', 'GB', 'TB'];

/** Unter 10 mit einer Nachkommastelle, sonst ganze Zahl: „1,2 GB“, „99 MB“. */
export function formatBytes(n: number): string {
  let value = n;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit++;
  }
  const text = unit > 0 && value < 10 ? value.toFixed(1).replace('.', T.dezimal) : String(Math.round(value));
  return `${text} ${UNITS[unit]}`;
}

/** Projektseite zum Lesen: „https://www.spotify.com/“ → „spotify.com“. Die Domain bleibt immer sichtbar. */
export function linkText(url: string): string {
  return url.replace(/^https?:\/\//, '').replace(/^www\./, '').replace(/\/$/, '');
}

export function formatDauer(sekunden: number): string {
  return `${Math.floor(sekunden / 60)}:${String(sekunden % 60).padStart(2, '0')}`;
}
