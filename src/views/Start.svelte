<script lang="ts">
  import * as api from '../lib/api';
  import type { FlatpakDuplicate, Overview, Program } from '../lib/api';
  import { T, schritt } from '../lib/texte';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import ProgramRow from '../components/ProgramRow.svelte';
  import SearchField from '../components/SearchField.svelte';

  let {
    overview,
    fehler,
    onsearch,
    onupdateall,
    onremove,
    onremoveflatpak,
    onsettings,
  }: {
    overview: Overview | null;
    fehler: string | null;
    onsearch: (query: string) => void;
    onupdateall: () => void;
    onremove: (p: Program) => void;
    onremoveflatpak: (d: FlatpakDuplicate) => void;
    onsettings: () => void;
  } = $props();

  let query = $state('');
  let zuEntfernen = $state<Program | null>(null);
  let auchEntfernt = $state<string[] | null>(null);
  let vorschauFehler = $state(false);

  /** Dialog öffnen und nachsehen, was mitgeht (z. B. Sprachpakete). */
  function fragen(p: Program) {
    zuEntfernen = p;
    auchEntfernt = null;
    vorschauFehler = false;
    api.removePreview(p).then((list) => {
      if (zuEntfernen === p) auchEntfernt = list;
    }, () => {
      if (zuEntfernen === p) vorschauFehler = true;
    });
  }

  function bestaetigt() {
    const p = zuEntfernen;
    zuEntfernen = null;
    if (p) onremove(p);
  }
</script>

<main class="seite">
  <SearchField bind:value={query} placeholder={T.start.suche} onsubmit={onsearch} />

  {#if fehler}
    <div class="hinweis">{fehler}</div>
  {/if}

  {#if !overview}
    <p class="leise">{schritt({ kind: 'Refreshing' })}</p>
  {:else}
    {#if overview.pending_count > 0}
      <div class="hinweis update-zeile">
        <span>{T.start.updates(overview.pending_count)}</span>
        <button class="btn btn-primary btn-sm" onclick={onupdateall}>{T.start.alleAktualisieren}</button>
      </div>
    {/if}
    {#each overview.flatpak_duplicates as dup (dup.flatpak_id)}
      <div class="hinweis update-zeile">
        <span>{T.start.flatpakDoppelt(dup.name)}</span>
        <button class="btn btn-secondary btn-sm" onclick={() => onremoveflatpak(dup)}>{T.start.flatpakEntfernen}</button>
      </div>
    {/each}

    <h5 class="abschnitt-titel">{T.start.deineProgramme}</h5>
    <ul class="programmliste">
      {#each overview.programs as program (program.desktop_file)}
        <ProgramRow {program} onremove={fragen} />
      {/each}
    </ul>
  {/if}

  <button class="textlink mitte" onclick={onsettings}>{T.start.einstellungen}</button>
</main>

{#if zuEntfernen}
  <ConfirmDialog
    titel={T.entfernen.titel(zuEntfernen.name)}
    text={vorschauFehler ? T.entfernen.vorschauFehler : auchEntfernt?.length ? `${T.entfernen.text} ${T.entfernen.auch(auchEntfernt)}` : T.entfernen.text}
    bestaetigen={T.start.entfernen}
    bereit={auchEntfernt !== null}
    onconfirm={bestaetigt}
    oncancel={() => (zuEntfernen = null)}
  />
{/if}
