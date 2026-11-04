// English texts. TypeScript checks that they match the German ones in texte.ts.

import type { Source, Step, TrustReason, UpdateState } from './api';
import type { Texte } from './texte';

export const en: Texte = {
  sprache: 'en',
  dezimal: '.',
  start: {
    updates: (n: number) => (n === 1 ? '1 package has updates.' : `${n} packages have updates.`),
    suche: 'Search or drop a file here',
    alleAktualisieren: 'Update all',
    deineProgramme: 'Your programs',
    entfernen: 'Remove',
    einstellungen: 'Settings',
    flatpakDoppelt: (name: string) => `${name} is installed twice, once natively and once as a Flatpak.`,
    flatpakEntfernen: 'Remove Flatpak version',
  },
  entfernen: {
    titel: (name: string) => `Remove ${name}?`,
    text: 'Your settings in your home folder stay in place.',
    auch: (pakete: string[]) => `This also removes: ${pakete.join(', ')}.`,
    vorschauFehler: "Terma couldn't check what else this would remove. Please try again in a moment.",
  },
  knopf: { abbrechen: 'Cancel' },
  suche: {
    empfohlen: (quelle: string) => `Recommended · ${quelle}`,
    installieren: 'Install',
    installiert: 'Installed',
    auchGefunden: 'Also found',
    nichts: (q: string) => `Terma found nothing for "${q}".`,
    aurNichtErreichbar: "The AUR can't be reached right now. Showing official sources only.",
  },
  datei: {
    aus: (datei: string) => `from ${datei}`,
    auchInQuelle: (name: string, source: Source) =>
      `${name} is also available ${source.kind === 'Aur' ? 'in the AUR' : 'in the CachyOS repository'}. Terma installs it from there, so updates arrive with the normal system update.`,
    nurAppImage: 'Set up the AppImage instead',
    selbstUpdate: (name: string) =>
      `${name} updates itself. As an AppImage it stays that way: Terma puts it in your Applications folder and sets up the start menu entry and icon.`,
    stattdessenQuelle: (source: Source) => `Install ${source.kind === 'Aur' ? 'from the AUR' : 'from the CachyOS repository'} instead`,
    einrichten: 'Set up',
    appimageText: 'Terma puts the AppImage in your Applications folder and sets up the start menu entry and icon.',
    nichtMoeglich: (name: string) => `Terma can't set up this file yet, and ${name} isn't in the repositories.`,
    keinAppImage: "This file isn't a valid AppImage.",
  },
  fortschritt: { laeuft: (dauer: string) => `Running for ${dauer}. Large downloads take a few minutes, just leave this window open.` },
  plan: { systemMit: (groesse: string) => `This also updates the system (${groesse}).` },
  warnung: {
    titel: (paket: string) => `Be careful with ${paket}`,
    text: 'This package comes from the AUR. Anyone can upload packages there, and nobody checks them first.',
    trotzdem: 'Install anyway',
  },
  fertig: {
    installiert: (name: string) => `${name} is installed. You'll find it in the start menu.`,
    neustart: 'Restart your computer so the new kernel can run.',
    aktualisiert: 'Everything is up to date.',
    restAktualisiert: 'Everything else is up to date.',
    aurNichtErreichbar: "The AUR couldn't be reached. Packages from there may be out of date.",
    appimagesGescheitert: (namen: string[]) => `Not updated: ${namen.join(', ')}.`,
    uebersprungen: (paket: string) => `${paket} was not updated:`,
    trotzdemAktualisieren: 'Update anyway',
  },
  fehler: {
    dbGesperrt: 'Another installation or update is already running. Wait until it finishes, then try again.',
    netz: 'No internet connection.',
    bauPgp: (paket: string) => `Building ${paket} failed: A signing key is missing.`,
    bauPruefsumme: (paket: string) => `Building ${paket} failed: The downloaded file doesn't match its checksum.`,
    bauQuellen: (paket: string) => `${paket} was not built: It downloads from a different address than the one Terma checked.`,
    bau: (paket: string) => `Building ${paket} failed.`,
    geschuetzt: (paket: string) => `${paket} is part of the system and stays installed.`,
    allgemein: "That didn't work.",
    details: 'Show details',
  },
  einstellungen: {
    titel: 'Settings',
    ohnePasswort: 'Install without a password',
    ohnePasswortText:
      'Terma can then install from official sources, remove programs and update without a password. Programs from the AUR still ask for your password. Malware on your computer could also use this permission.',
    zurueck: 'Back',
  },
  quelle(source: Source): string {
    return { Repo: 'CachyOS repository', Aur: 'AUR', AppImage: 'AppImage' }[source.kind];
  },
  updateText(state: UpdateState): string | null {
    return { None: null, Ready: 'Update ready', SelfUpdating: 'Updates itself', NotPossible: 'No updates possible' }[state];
  },
  grund(r: TrustReason): string {
    switch (r.kind) {
      case 'TooNew':
        return `This package has only existed for ${r.days} days.`;
      case 'FewVotes':
        return `It only has ${r.votes} votes.`;
      case 'LowPopularity':
        return 'Hardly anyone uses it.';
      case 'Orphaned':
        return 'Nobody is looking after this package right now.';
      case 'ImitatesName':
        return `The name imitates "${r.original}". Malicious packages were disguised like this in 2025.`;
      case 'SourceMismatch':
        return `It downloads the program from ${r.source}, but the project lives at ${r.project}.`;
      case 'MaintainerChanged':
        return 'This package has a new maintainer since you installed it.';
      case 'NewSource':
        return `This package now downloads from a new address (${r.domain}).`;
      case 'RiskyDependency':
        return `It now also needs ${r.package} from the AUR, and that package does not pass the check.`;
    }
  },
  schritt(s: Step): string {
    switch (s.kind) {
      case 'Refreshing':
        return 'Loading package lists';
      case 'UpdatingSystem':
        return 'Updating the system';
      case 'Building':
        return `Building ${s.package}`;
      case 'Installing':
        return `Installing ${s.package}`;
      case 'SettingUpAppImage':
        return `Setting up ${s.name}`;
      case 'Removing':
        return `Removing ${s.name}`;
    }
  },
};
