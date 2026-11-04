<script lang="ts">
  import { T } from '../lib/texte';

  let {
    titel,
    text,
    bestaetigen,
    bereit = true,
    onconfirm,
    oncancel,
  }: { titel: string; text: string; bestaetigen: string; bereit?: boolean; onconfirm: () => void; oncancel: () => void } = $props();
</script>

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && oncancel()}>
  <div class="dialog facette" role="dialog" aria-modal="true" aria-label={titel}>
    <h4>{titel}</h4>
    <p>{text}</p>
    <div class="knopfreihe">
      <button class="btn btn-secondary" onclick={oncancel}>{T.knopf.abbrechen}</button>
      <button class="btn btn-primary" disabled={!bereit} onclick={onconfirm}>{bestaetigen}</button>
    </div>
  </div>
</div>
