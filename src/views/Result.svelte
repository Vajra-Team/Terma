<script lang="ts">
  import type { FailedUpdate, SkippedUpdate } from '../lib/api';
  import { T, bauFehler, grund } from '../lib/texte';

  let {
    ok,
    text,
    neustart = false,
    uebersprungen = [],
    gescheitert = [],
    hinweise = [],
    details = null,
    onforce,
    onback,
  }: {
    ok: boolean;
    text: string;
    neustart?: boolean;
    uebersprungen?: SkippedUpdate[];
    gescheitert?: FailedUpdate[];
    hinweise?: string[];
    details?: string | null;
    onforce: (pkg: string) => void;
    onback: () => void;
  } = $props();
  let offen = $state(false);
</script>

<main class="seite">
  {#if ok}<h3>{text}</h3>{:else}<div class="hinweis">{text}</div>{/if}
  {#if neustart}<div class="hinweis">{T.fertig.neustart}</div>{/if}

  {#each gescheitert as f (f.package)}
    <div class="hinweis">{bauFehler(f.kind, f.package)}</div>
  {/each}

  {#each hinweise as h (h)}
    <div class="hinweis">{h}</div>
  {/each}

  {#each uebersprungen as s (s.package)}
    <div class="facette karte">
      <h5>{T.fertig.uebersprungen(s.package)}</h5>
      <ul class="gruende">
        {#each s.reasons as r, i (i)}<li><span class="raute"></span><span>{grund(r)}</span></li>{/each}
      </ul>
      <div><button class="btn btn-secondary btn-sm" onclick={() => onforce(s.package)}>{T.fertig.trotzdemAktualisieren}</button></div>
    </div>
  {/each}

  {#if details}
    <button class="textlink links" onclick={() => (offen = !offen)}>{T.fehler.details}</button>
    {#if offen}<pre class="log">{details}</pre>{/if}
  {/if}

  <button class="textlink mitte" onclick={onback}>{T.einstellungen.zurueck}</button>
</main>
