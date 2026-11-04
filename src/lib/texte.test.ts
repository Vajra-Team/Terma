import { describe, expect, it, vi } from 'vitest';
import { de, grund, schritt, texteFuer } from './texte';
import { en } from './texte.en';

describe('texteFuer', () => {
  it('nimmt Deutsch nur bei deutscher Systemsprache, sonst Englisch', () => {
    expect(texteFuer('de-DE')).toBe(de);
    expect(texteFuer('de')).toBe(de);
    expect(texteFuer('DE-at')).toBe(de);
    expect(texteFuer('en-US')).toBe(en);
    expect(texteFuer('fr-FR')).toBe(en);
    expect(texteFuer('')).toBe(en);
  });
});

function festeTexte(wert: unknown): string[] {
  if (typeof wert === 'string') return [wert];
  if (wert && typeof wert === 'object') return Object.values(wert).flatMap(festeTexte);
  return [];
}

describe('englische Texte', () => {
  it('enthalten nichts Deutsches', () => {
    expect(festeTexte(en).filter((t) => /[äöüß„“]/i.test(t))).toEqual([]);
  });

  it('sagen dasselbe wie die deutschen', () => {
    expect(en.start.alleAktualisieren).toBe('Update all');
    expect(en.start.updates(1)).toBe('1 package has updates.');
    expect(en.start.updates(3)).toBe('3 packages have updates.');
    expect(en.entfernen.auch(['a', 'b'])).toBe('This also removes: a, b.');
    expect(en.quelle({ kind: 'Aur' })).toBe('AUR');
    expect(en.grund({ kind: 'FewVotes', votes: 11 })).toBe('It only has 11 votes.');
    expect(en.schritt({ kind: 'Building', package: 'foo' })).toBe('Building foo');
    expect(en.updateText('Ready')).toBe('Update ready');
  });
});

describe('deutsche Hilfsfunktionen', () => {
  it('bleiben wie sie sind', () => {
    expect(de.start.updates(1)).toBe('Für 1 Paket gibt es Updates.');
    expect(de.start.updates(3)).toBe('Für 3 Pakete gibt es Updates.');
    expect(grund({ kind: 'FewVotes', votes: 11 })).toBe('Es hat erst 11 Stimmen.');
    expect(schritt({ kind: 'UpdatingSystem' })).toBe('System wird aktualisiert');
  });
});

describe('Systemsprache', () => {
  it('bestimmt Texte und Dezimaltrenner', async () => {
    vi.resetModules();
    vi.stubGlobal('navigator', { language: 'en-US' });
    const { formatBytes } = await import('./format');
    const { T } = await import('./texte');
    expect(formatBytes(1_234_567_890)).toBe('1.2 GB');
    expect(T.sprache).toBe('en');
    vi.stubGlobal('navigator', { language: 'de-DE' });
    vi.resetModules();
  });
});
