<script lang="ts">
  import { formatDauer } from '../lib/format';
  import { T } from '../lib/texte';

  let { titel, zeilen }: { titel: string; zeilen: string[] } = $props();
  let offen = $state(false);
  // pacman meldet beim Laden nichts: Die Zeit zeigt, dass das Fenster nicht hängt.
  let sekunden = $state(0);
  $effect(() => {
    const takt = setInterval(() => sekunden++, 1000);
    return () => clearInterval(takt);
  });
</script>

<main class="seite">
  <h3>{titel}</h3>
  <p class="daten">{T.fortschritt.laeuft(formatDauer(sekunden))}</p>
  {#if zeilen.length > 0}
    <p class="daten letzte">{zeilen[zeilen.length - 1]}</p>
    <button class="textlink links" onclick={() => (offen = !offen)}>{T.fehler.details}</button>
    {#if offen}<pre class="log">{zeilen.join('\n')}</pre>{/if}
  {/if}
</main>
