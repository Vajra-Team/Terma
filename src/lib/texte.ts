// Alle sichtbaren Texte von Terma. Wörtlich aus dem Umsetzungsplan, Task 12.
// Die englische Fassung steht in texte.en.ts.

import type { BuildFailure, Source, Step, TrustReason, UpdateState } from './api';
import { en } from './texte.en';

export const de = {
  sprache: 'de',
  dezimal: ',',
  start: {
    updates: (n: number) => (n === 1 ? 'Für 1 Paket gibt es Updates.' : `Für ${n} Pakete gibt es Updates.`),
    suche: 'Suchen oder Datei reinziehen',
    alleAktualisieren: 'Alle aktualisieren',
    deineProgramme: 'Deine Programme',
    entfernen: 'Entfernen',
    einstellungen: 'Einstellungen',
    flatpakDoppelt: (name: string) => `${name} ist doppelt installiert, einmal nativ und einmal als Flatpak.`,
    flatpakEntfernen: 'Flatpak-Version entfernen',
  },
  entfernen: {
    titel: (name: string) => `${name} entfernen?`,
    text: 'Deine Einstellungen im Home-Ordner bleiben erhalten.',
    auch: (pakete: string[]) => `Dabei wird auch entfernt: ${pakete.join(', ')}.`,
    vorschauFehler: 'Terma konnte nicht prüfen, was dabei mit entfernt würde. Versuch es gleich noch einmal.',
  },
  knopf: { abbrechen: 'Abbrechen' },
  suche: {
    empfohlen: (quelle: string) => `Empfohlen · ${quelle}`,
    installieren: 'Installieren',
    installiert: 'Installiert',
    auchGefunden: 'Auch gefunden',
    nichts: (q: string) => `Für „${q}“ hat Terma nichts gefunden.`,
    aurNichtErreichbar: 'Das AUR ist gerade nicht erreichbar. Gezeigt werden nur die offiziellen Quellen.',
  },
  datei: {
    aus: (datei: string) => `aus ${datei}`,
    auchInQuelle: (name: string, source: Source) =>
      `${name} gibt es auch ${source.kind === 'Aur' ? 'im AUR' : 'in der CachyOS-Paketquelle'}. Terma installiert es von dort, dann kommen Updates mit dem normalen Systemupdate.`,
    nurAppImage: 'Stattdessen nur das AppImage einrichten',
    selbstUpdate: (name: string) =>
      `${name} aktualisiert sich selbst. Als AppImage bleibt das so: Terma legt es in deinen Programme-Ordner und richtet Startmenü-Eintrag und Icon ein.`,
    stattdessenQuelle: (source: Source) => `Stattdessen ${source.kind === 'Aur' ? 'aus dem AUR' : 'aus der CachyOS-Paketquelle'} installieren`,
    einrichten: 'Einrichten',
    appimageText: 'Terma legt das AppImage in deinen Programme-Ordner und richtet Startmenü-Eintrag und Icon ein.',
    nichtMoeglich: (name: string) => `Diese Datei kann Terma noch nicht einrichten, und in den Paketquellen gibt es ${name} nicht.`,
    keinAppImage: 'Diese Datei ist kein gültiges AppImage.',
  },
  fortschritt: { laeuft: (dauer: string) => `Läuft seit ${dauer}. Große Downloads dauern ein paar Minuten, lass das Fenster einfach offen.` },
  plan: { systemMit: (groesse: string) => `Dabei wird auch das System aktualisiert (${groesse}).` },
  warnung: {
    titel: (paket: string) => `Vorsicht bei ${paket}`,
    text: 'Dieses Paket kommt aus dem AUR. Dort kann jeder Pakete hochladen, niemand prüft sie vorher.',
    trotzdem: 'Trotzdem installieren',
  },
  fertig: {
    installiert: (name: string) => `${name} ist installiert. Du findest es im Startmenü.`,
    neustart: 'Starte den Rechner neu, damit der neue Kernel läuft.',
    aktualisiert: 'Alles ist aktuell.',
    restAktualisiert: 'Der Rest ist aktuell.',
    aurNichtErreichbar: 'Das AUR war nicht erreichbar. Pakete von dort sind vielleicht nicht aktuell.',
    appimagesGescheitert: (namen: string[]) => `Nicht aktualisiert: ${namen.join(', ')}.`,
    uebersprungen: (paket: string) => `${paket} wurde nicht aktualisiert:`,
    trotzdemAktualisieren: 'Trotzdem aktualisieren',
  },
  fehler: {
    dbGesperrt: 'Gerade läuft schon eine andere Installation oder ein Update. Warte, bis sie fertig ist, und versuch es dann noch einmal.',
    netz: 'Keine Verbindung zum Internet.',
    bauPgp: (paket: string) => `Der Bau von ${paket} ist gescheitert: Ein Signaturschlüssel fehlt.`,
    bauPruefsumme: (paket: string) => `Der Bau von ${paket} ist gescheitert: Die heruntergeladene Datei passt nicht zur Prüfsumme.`,
    bauQuellen: (paket: string) => `${paket} wurde nicht gebaut: Es lädt von einer anderen Adresse als der, die Terma geprüft hat.`,
    bau: (paket: string) => `Der Bau von ${paket} ist gescheitert.`,
    geschuetzt: (paket: string) => `${paket} gehört zum System und bleibt installiert.`,
    allgemein: 'Das hat nicht geklappt.',
    details: 'Details zeigen',
  },
  einstellungen: {
    titel: 'Einstellungen',
    ohnePasswort: 'Ohne Passwort installieren',
    ohnePasswortText:
      'Terma darf dann ohne Passwort aus den offiziellen Quellen installieren, Programme entfernen und aktualisieren. Programme aus dem AUR fragen weiter nach dem Passwort. Ein Schadprogramm auf deinem Rechner könnte diese Erlaubnis ebenfalls nutzen.',
    zurueck: 'Zurück',
  },
  quelle(source: Source): string {
    return { Repo: 'CachyOS-Paketquelle', Aur: 'AUR', AppImage: 'AppImage' }[source.kind];
  },

  updateText(state: UpdateState): string | null {
    return { None: null, Ready: 'Update bereit', SelfUpdating: 'Aktualisiert sich selbst', NotPossible: 'Keine Updates möglich' }[state];
  },

  grund(r: TrustReason): string {
    switch (r.kind) {
      case 'TooNew':
        return `Das Paket gibt es erst seit ${r.days} Tagen.`;
      case 'FewVotes':
        return `Es hat erst ${r.votes} Stimmen.`;
      case 'LowPopularity':
        return 'Es wird kaum genutzt.';
      case 'Orphaned':
        return 'Niemand kümmert sich gerade um das Paket.';
      case 'ImitatesName':
        return `Der Name ahmt „${r.original}“ nach. So waren 2025 Schadpakete getarnt.`;
      case 'SourceMismatch':
        return `Es lädt das Programm von ${r.source}, das Projekt liegt aber auf ${r.project}.`;
      case 'MaintainerChanged':
        return 'Das Paket hat seit der Installation einen neuen Betreuer.';
      case 'NewSource':
        return `Das Paket lädt jetzt von einer neuen Adresse (${r.domain}).`;
      case 'RiskyDependency':
        return `Es braucht jetzt auch ${r.package} aus dem AUR, und das fällt bei der Prüfung auf.`;
    }
  },

  schritt(s: Step): string {
    switch (s.kind) {
      case 'Refreshing':
        return 'Paketlisten werden geladen';
      case 'UpdatingSystem':
        return 'System wird aktualisiert';
      case 'Building':
        return `${s.package} wird gebaut`;
      case 'Installing':
        return `${s.package} wird installiert`;
      case 'SettingUpAppImage':
        return `${s.name} wird eingerichtet`;
      case 'Removing':
        return `${s.name} wird entfernt`;
    }
  },
};

export type Texte = typeof de;

/** Deutsch bei deutscher Systemsprache, sonst Englisch. */
export function texteFuer(sprache: string): Texte {
  return sprache.toLowerCase().startsWith('de') ? de : en;
}

export const T = texteFuer(navigator.language);
export const { quelle, updateText, grund, schritt } = T;

export function bauFehler(kind: BuildFailure, paket: string): string {
  return { PgpKeyUnknown: T.fehler.bauPgp, ChecksumMismatch: T.fehler.bauPruefsumme, SourcesChanged: T.fehler.bauQuellen, Other: T.fehler.bau }[kind](paket);
}
