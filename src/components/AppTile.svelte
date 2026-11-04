<script lang="ts">
  import { iconSrc } from '../lib/api';

  let { name, icon = null, gross = false }: { name: string; icon?: string | null; gross?: boolean } = $props();
  let failed = $state(false);
  const src = $derived(failed ? null : iconSrc(icon));
</script>

<div class="app-kachel" class:gross>
  {#if src}
    <img {src} alt="" onerror={() => (failed = true)} />
  {:else}
    {name.charAt(0).toUpperCase()}
  {/if}
</div>
