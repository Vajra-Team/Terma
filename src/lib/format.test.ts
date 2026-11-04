import { describe, expect, it } from 'vitest';
import { formatBytes, formatDauer, linkText } from './format';

describe('formatBytes', () => {
  it('rundet wie in den Entwürfen', () => {
    expect(formatBytes(1_234_567_890)).toBe('1,2 GB');
    expect(formatBytes(98_500_000)).toBe('99 MB');
    expect(formatBytes(9_400_000)).toBe('9,4 MB');
    expect(formatBytes(2_170_552)).toBe('2,2 MB');
    expect(formatBytes(512_000)).toBe('512 kB');
  });
});

describe('linkText', () => {
  it('zeigt die Adresse ohne Schema, www und Schrägstrich am Ende', () => {
    expect(linkText('https://github.com/pingdotgg/t3code')).toBe('github.com/pingdotgg/t3code');
    expect(linkText('https://www.spotify.com')).toBe('spotify.com');
    expect(linkText('https://code.visualstudio.com/')).toBe('code.visualstudio.com');
    expect(linkText('http://example.org/a/')).toBe('example.org/a');
  });
});

import { T } from './texte';

describe('entfernen.auch', () => {
  it('nennt mitentfernte Pakete', () => {
    expect(T.entfernen.auch(['firefox-i18n-de'])).toBe('Dabei wird auch entfernt: firefox-i18n-de.');
    expect(T.entfernen.auch(['a', 'b'])).toBe('Dabei wird auch entfernt: a, b.');
  });
});

describe('formatDauer', () => {
  it('zeigt Minuten und Sekunden', () => {
    expect(formatDauer(0)).toBe('0:00');
    expect(formatDauer(65)).toBe('1:05');
    expect(formatDauer(3725)).toBe('62:05');
  });
});
