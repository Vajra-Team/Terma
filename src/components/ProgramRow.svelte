<script lang="ts">
  import type { Program } from '../lib/api';
  import { T, quelle, updateText } from '../lib/texte';
  import AppTile from './AppTile.svelte';

  let { program, onremove }: { program: Program; onremove: (p: Program) => void } = $props();
  const update = $derived(updateText(program.update));
</script>

<li>
  <AppTile name={program.name} icon={program.icon} />
  <div class="txt">
    <strong>{program.name}</strong>
    <span class="badge" class:badge-messing={program.update === 'Ready'}>
      {quelle(program.source)}{update ? ` · ${update}` : ''}
    </span>
  </div>
  <button class="btn btn-secondary btn-sm" onclick={() => onremove(program)}>{T.start.entfernen}</button>
</li>
