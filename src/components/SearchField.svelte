<script lang="ts">
  let {
    value = $bindable(''),
    placeholder,
    onsubmit,
  }: { value?: string; placeholder: string; onsubmit: (query: string) => void } = $props();
</script>

<form
  class="suchfeld"
  role="search"
  onsubmit={(e) => {
    e.preventDefault();
    if (value.trim()) onsubmit(value.trim());
  }}
>
  <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
    <circle cx="8.5" cy="8.5" r="6" /><path d="M13 13l5 5" />
  </svg>
  <input type="search" bind:value {placeholder} aria-label={placeholder} spellcheck="false" />
</form>
