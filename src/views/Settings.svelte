<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from '../lib/api';
  import { fehlerText } from '../lib/flow';
  import { T } from '../lib/texte';

  let { onback }: { onback: () => void } = $props();
  let passwordless = $state(false);
  let busy = $state(false);
  let fehler = $state<string | null>(null);

  onMount(async () => {
    passwordless = (await api.getSettings()).passwordless;
  });

  async function umschalten() {
    busy = true;
    fehler = null;
    const next = !passwordless;
    try {
      await api.setPasswordless(next);
      passwordless = next;
    } catch (e) {
      // Passwortfenster abgebrochen: Einstellung bleibt, wie sie war, ohne Meldung.
      fehler = fehlerText(e);
    } finally {
      busy = false;
    }
  }
</script>

<main class="seite">
  <h2>{T.einstellungen.titel}</h2>
  <div class="facette einstellung">
    <div class="txt">
      <h5>{T.einstellungen.ohnePasswort}</h5>
      <p>{T.einstellungen.ohnePasswortText}</p>
    </div>
    <button
      class="schalter"
      role="switch"
      aria-checked={passwordless}
      aria-label={T.einstellungen.ohnePasswort}
      disabled={busy}
      onclick={umschalten}
    ></button>
  </div>
  {#if fehler}<div class="hinweis">{fehler}</div>{/if}
  <button class="textlink mitte" onclick={onback}>{T.einstellungen.zurueck}</button>
</main>
