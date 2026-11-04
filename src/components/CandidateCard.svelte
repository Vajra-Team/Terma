<script lang="ts">
  import { openUrl, type Candidate } from '../lib/api';
  import { formatBytes, linkText } from '../lib/format';
  import { T, quelle } from '../lib/texte';
  import AppTile from './AppTile.svelte';

  let { candidate, oninstall }: { candidate: Candidate; oninstall: (c: Candidate) => void } = $props();
</script>

<div class="facette karte">
  <div class="app">
    <AppTile name={candidate.display_name} icon={candidate.icon} gross />
    <div>
      <h4>{candidate.display_name}</h4>
      {#if candidate.description}<p class="sub">{candidate.description}</p>{/if}
    </div>
  </div>
  <div class="meta">
    <span class="badge badge-messing">{T.suche.empfohlen(quelle(candidate.source))}</span>
    {#if candidate.download_size}<span class="daten">{formatBytes(candidate.download_size)}</span>{/if}
    {#if candidate.url}<button class="textlink adresse" onclick={() => openUrl(candidate.url!)}>{linkText(candidate.url)}</button>{/if}
  </div>
  {#if candidate.installed}
    <button class="btn btn-secondary voll" disabled>{T.suche.installiert}</button>
  {:else}
    <button class="btn btn-primary voll" onclick={() => oninstall(candidate)}>{T.suche.installieren}</button>
  {/if}
</div>
