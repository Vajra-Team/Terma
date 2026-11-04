<script lang="ts">
  import { untrack } from 'svelte';
  import { openUrl, type Candidate, type SearchResult } from '../lib/api';
  import { linkText } from '../lib/format';
  import { T, quelle, schritt } from '../lib/texte';
  import AppTile from '../components/AppTile.svelte';
  import CandidateCard from '../components/CandidateCard.svelte';
  import SearchField from '../components/SearchField.svelte';

  let {
    query,
    result,
    fehler,
    onsearch,
    oninstall,
    onback,
  }: {
    query: string;
    result: SearchResult | null;
    fehler: string | null;
    onsearch: (q: string) => void;
    oninstall: (c: Candidate) => void;
    onback: () => void;
  } = $props();

  // Startwert ist die Anfrage, danach gehört das Feld dem Nutzer.
  let eingabe = $state(untrack(() => query));
</script>

<main class="seite">
  <SearchField bind:value={eingabe} placeholder={T.start.suche} onsubmit={onsearch} />

  {#if fehler}
    <div class="hinweis">{fehler}</div>
  {:else if !result}
    <p class="leise">{schritt({ kind: 'Refreshing' })}</p>
  {:else}
    {#if result.aur_unavailable}<div class="hinweis">{T.suche.aurNichtErreichbar}</div>{/if}
    {#if result.recommended}
      <CandidateCard candidate={result.recommended} {oninstall} />
    {:else}
      <div class="hinweis">{T.suche.nichts(query)}</div>
    {/if}
    {#if result.others.length > 0}
      <h5 class="abschnitt-titel">{T.suche.auchGefunden}</h5>
      <ul class="programmliste">
        {#each result.others as c (c.name)}
          <li>
            <AppTile name={c.display_name} icon={c.icon} />
            <div class="txt">
              <strong>{c.display_name}</strong>
              {#if c.description}<p class="sub">{c.description}</p>{/if}
              <div class="meta">
                <span class="badge">{quelle(c.source)}</span>
                {#if c.url}<button class="textlink adresse" onclick={() => openUrl(c.url!)}>{linkText(c.url)}</button>{/if}
              </div>
            </div>
            {#if c.installed}
              <button class="btn btn-secondary btn-sm" disabled>{T.suche.installiert}</button>
            {:else}
              <button class="btn btn-secondary btn-sm" onclick={() => oninstall(c)}>{T.suche.installieren}</button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  <button class="textlink mitte" onclick={onback}>{T.einstellungen.zurueck}</button>
</main>
