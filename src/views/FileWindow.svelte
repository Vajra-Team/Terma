<script lang="ts">
  import type { Candidate, FileInspection } from '../lib/api';
  import { dateiEmpfehlung } from '../lib/flow';
  import { T, schritt } from '../lib/texte';
  import AppTile from '../components/AppTile.svelte';

  let {
    path,
    inspection,
    fehler,
    oninstall,
    onsetup,
    onback,
  }: {
    path: string;
    inspection: FileInspection | null;
    fehler: string | null;
    oninstall: (c: Candidate) => void;
    onsetup: () => void;
    onback: () => void;
  } = $props();

  const datei = $derived(path.split('/').pop() ?? path);
</script>

<main class="seite">
  {#if fehler}
    <div class="hinweis">{fehler}</div>
  {:else if !inspection}
    <p class="leise">{schritt({ kind: 'Refreshing' })}</p>
  {:else}
    <div class="app">
      <AppTile name={inspection.app_name} icon={inspection.icon} gross />
      <div>
        <h4>{inspection.app_name}</h4>
        <p class="sub">{T.datei.aus(datei)}</p>
      </div>
    </div>

    {@const empfehlung = dateiEmpfehlung(inspection)}
    {#if empfehlung === 'quelle' && inspection.source_match}
      {@const match = inspection.source_match}
      <div class="hinweis">{T.datei.auchInQuelle(inspection.app_name, match.source)}</div>
      <button class="btn btn-primary voll" onclick={() => oninstall(match)}>{T.suche.installieren}</button>
      {#if inspection.kind === 'AppImage'}
        <button class="textlink mitte" onclick={onsetup}>{T.datei.nurAppImage}</button>
      {/if}
    {:else if empfehlung === 'appimage'}
      <p class="sub">{inspection.self_updating ? T.datei.selbstUpdate(inspection.app_name) : T.datei.appimageText}</p>
      <button class="btn btn-primary voll" onclick={onsetup}>{T.datei.einrichten}</button>
      {#if inspection.source_match}
        {@const match = inspection.source_match}
        <button class="textlink mitte" onclick={() => oninstall(match)}>{T.datei.stattdessenQuelle(match.source)}</button>
      {/if}
    {:else}
      <div class="hinweis">{T.datei.nichtMoeglich(inspection.app_name)}</div>
    {/if}
  {/if}

  <button class="textlink mitte" onclick={onback}>{T.knopf.abbrechen}</button>
</main>
