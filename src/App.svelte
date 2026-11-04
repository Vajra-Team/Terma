<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from './lib/api';
  import { fehlerDetails, fehlerText, nextAfterPlan } from './lib/flow';
  import { T, schritt } from './lib/texte';
  import FileWindow from './views/FileWindow.svelte';
  import InstallConfirm from './views/InstallConfirm.svelte';
  import Progress from './views/Progress.svelte';
  import Result from './views/Result.svelte';
  import Search from './views/Search.svelte';
  import Settings from './views/Settings.svelte';
  import Start from './views/Start.svelte';

  type View =
    | { kind: 'start' }
    | { kind: 'settings' }
    | { kind: 'search'; query: string; result: api.SearchResult | null; fehler: string | null }
    | { kind: 'file'; path: string; inspection: api.FileInspection | null; fehler: string | null }
    | { kind: 'confirm'; plan: api.InstallPlan; zurueck: View; name: string }
    | { kind: 'progress'; titel: string }
    | { kind: 'result'; ok: boolean; text: string; neustart: boolean; uebersprungen: api.SkippedUpdate[]; gescheitert?: api.FailedUpdate[]; hinweise?: string[]; details: string | null };

  let view = $state<View>({ kind: 'start' });
  let overview = $state<api.Overview | null>(null);
  let startFehler = $state<string | null>(null);
  let zeilen = $state<string[]>([]);

  async function laden() {
    startFehler = null;
    try {
      overview = await api.overview();
    } catch (e) {
      startFehler = fehlerText(e);
      overview ??= { pending_count: 0, programs: [], flatpak_duplicates: [] };
    }
  }

  function zurStart() {
    view = { kind: 'start' };
    laden();
  }

  /** Führt einen Vorgang mit Fortschrittsansicht aus. Abbruch im Passwortfenster führt still zurück. */
  async function vorgang<R>(titel: string, zurueck: View, run: () => Promise<R>, erfolg: (r: R) => View) {
    zeilen = [];
    view = { kind: 'progress', titel };
    try {
      view = erfolg(await run());
    } catch (e) {
      const text = fehlerText(e);
      view = text === null ? zurueck : { kind: 'result', ok: false, text, neustart: false, uebersprungen: [], details: fehlerDetails(e) };
    }
  }

  async function suchen(query: string) {
    view = { kind: 'search', query, result: null, fehler: null };
    try {
      const result = await api.search(query);
      if (view.kind === 'search' && view.query === query) view = { ...view, result };
    } catch (e) {
      if (view.kind === 'search') view = { ...view, fehler: fehlerText(e) ?? T.fehler.allgemein };
    }
  }

  async function dateiOeffnen(path: string) {
    view = { kind: 'file', path, inspection: null, fehler: null };
    try {
      const inspection = await api.inspectFile(path);
      if (view.kind === 'file' && view.path === path) view = { ...view, inspection };
    } catch (e) {
      if (view.kind === 'file') view = { ...view, fehler: fehlerText(e) ?? T.fehler.allgemein };
    }
  }

  /** `name`: so heißt das Programm für Jamie, bei Dateien der Name aus der Datei statt des Paketnamens. */
  async function installieren(candidate: api.Candidate, fromFile: string | null = null, name = candidate.display_name) {
    const zurueck = view;
    zeilen = [];
    view = { kind: 'progress', titel: schritt({ kind: 'Refreshing' }) };
    let plan: api.InstallPlan;
    try {
      plan = await api.planInstall(candidate, fromFile);
    } catch (e) {
      const text = fehlerText(e);
      view = text === null ? zurueck : { kind: 'result', ok: false, text, neustart: false, uebersprungen: [], details: fehlerDetails(e) };
      return;
    }
    if (nextAfterPlan(plan) === 'confirm') view = { kind: 'confirm', plan, zurueck, name };
    else ausfuehren(plan, zurueck, name);
  }

  function ausfuehren(plan: api.InstallPlan, zurueck: View, name: string) {
    vorgang(schritt({ kind: 'Installing', package: name }), zurueck, () => api.install(plan), (o) => ({
      kind: 'result',
      ok: true,
      text: T.fertig.installiert(name),
      neustart: o.restart_recommended,
      uebersprungen: [],
      details: null,
    }));
  }

  function einrichten(path: string, name: string) {
    vorgang(schritt({ kind: 'SettingUpAppImage', name }), view, () => api.setupAppImage(path), (app) => ({
      kind: 'result',
      ok: true,
      text: T.fertig.installiert(app.name),
      neustart: false,
      uebersprungen: [],
      details: null,
    }));
  }

  function entfernen(p: api.Program) {
    vorgang(schritt({ kind: 'Removing', name: p.name }), { kind: 'start' }, () => api.remove(p), () => {
      laden();
      return { kind: 'start' };
    });
  }

  async function flatpakEntfernen(d: api.FlatpakDuplicate) {
    try {
      await api.removeFlatpak(d);
    } catch (e) {
      startFehler = fehlerText(e);
    }
    laden();
  }

  function alleAktualisieren() {
    vorgang(schritt({ kind: 'UpdatingSystem' }), { kind: 'start' }, () => api.updateAll(), (report) => ({
      kind: 'result',
      ok: true,
      text: report.skipped.length + report.failed.length + report.appimages_failed.length > 0 || report.aur_unreachable ? T.fertig.restAktualisiert : T.fertig.aktualisiert,
      neustart: report.restart_recommended,
      uebersprungen: report.skipped,
      gescheitert: report.failed,
      hinweise: [
        ...(report.aur_unreachable ? [T.fertig.aurNichtErreichbar] : []),
        ...(report.appimages_failed.length > 0 ? [T.fertig.appimagesGescheitert(report.appimages_failed)] : []),
      ],
      details: null,
    }));
  }

  function trotzdemAktualisieren(pkg: string) {
    vorgang(schritt({ kind: 'Building', package: pkg }), view, () => api.forceUpdateAur([pkg]), () => ({
      kind: 'result',
      ok: true,
      text: T.fertig.aktualisiert,
      neustart: false,
      uebersprungen: [],
      details: null,
    }));
  }

  onMount(() => {
    const abmelden: Promise<() => void>[] = [
      api.onProgress((p) => {
        if (p.kind === 'Line') zeilen = [...zeilen.slice(-499), p.data];
        else if (view.kind === 'progress') view = { kind: 'progress', titel: schritt(p.data) };
      }),
      api.onOpenFile(dateiOeffnen),
      api.onFileDrop(dateiOeffnen),
    ];
    api.startupFile().then((path) => (path ? dateiOeffnen(path) : laden()));
    return () => abmelden.forEach((a) => a.then((f) => f()));
  });
</script>

{#if view.kind === 'settings'}
  <Settings onback={zurStart} />
{:else if view.kind === 'search'}
  <Search query={view.query} result={view.result} fehler={view.fehler} onsearch={suchen} oninstall={(c) => installieren(c)} onback={zurStart} />
{:else if view.kind === 'file'}
  {@const file = view}
  <FileWindow
    path={file.path}
    inspection={file.inspection}
    fehler={file.fehler}
    oninstall={(c) => installieren(c, file.path, file.inspection?.app_name ?? c.display_name)}
    onsetup={() => einrichten(file.path, file.inspection?.app_name ?? '')}
    onback={zurStart}
  />
{:else if view.kind === 'confirm'}
  {@const confirm = view}
  <InstallConfirm plan={confirm.plan} onconfirm={() => ausfuehren(confirm.plan, confirm.zurueck, confirm.name)} oncancel={() => (view = confirm.zurueck)} />
{:else if view.kind === 'progress'}
  <Progress titel={view.titel} {zeilen} />
{:else if view.kind === 'result'}
  <Result
    ok={view.ok}
    text={view.text}
    neustart={view.neustart}
    uebersprungen={view.uebersprungen}
    gescheitert={view.gescheitert}
    hinweise={view.hinweise}
    details={view.details}
    onforce={trotzdemAktualisieren}
    onback={zurStart}
  />
{:else}
  <Start
    {overview}
    fehler={startFehler}
    onsearch={suchen}
    onupdateall={alleAktualisieren}
    onremove={entfernen}
    onremoveflatpak={flatpakEntfernen}
    onsettings={() => (view = { kind: 'settings' })}
  />
{/if}
