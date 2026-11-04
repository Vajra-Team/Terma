<script lang="ts">
  import type { InstallPlan } from '../lib/api';
  import { formatBytes } from '../lib/format';
  import { T, grund } from '../lib/texte';
  import AppTile from '../components/AppTile.svelte';

  let { plan, onconfirm, oncancel }: { plan: InstallPlan; onconfirm: () => void; oncancel: () => void } = $props();
  const gewarnt = $derived(plan.warnings.length > 0);
</script>

<main class="seite">
  {#if gewarnt}
    <h3>{T.warnung.titel(plan.warnings.length === 1 ? plan.warnings[0].package : plan.candidate.name)}</h3>
    <p class="sub">{T.warnung.text}</p>
    {#each plan.warnings as w (w.package)}
      <div class="facette karte">
        {#if plan.warnings.length > 1}<h5>{w.package}</h5>{/if}
        <ul class="gruende">
          {#each w.reasons as r, i (i)}
            <li><span class="raute"></span><span>{grund(r)}</span></li>
          {/each}
        </ul>
      </div>
    {/each}
  {:else}
    <div class="app">
      <AppTile name={plan.candidate.display_name} gross />
      <div>
        <h4>{plan.candidate.display_name}</h4>
        {#if plan.candidate.description}<p class="sub">{plan.candidate.description}</p>{/if}
      </div>
    </div>
  {/if}

  {#if plan.system_update}
    <div class="hinweis">{T.plan.systemMit(formatBytes(plan.system_update.download_bytes))}</div>
  {/if}

  <div class="knopfreihe">
    {#if gewarnt}
      <button class="btn btn-secondary" onclick={onconfirm}>{T.warnung.trotzdem}</button>
      <button class="btn btn-primary" onclick={oncancel}>{T.knopf.abbrechen}</button>
    {:else}
      <button class="btn btn-secondary" onclick={oncancel}>{T.knopf.abbrechen}</button>
      <button class="btn btn-primary" onclick={onconfirm}>{T.suche.installieren}</button>
    {/if}
  </div>
</main>
