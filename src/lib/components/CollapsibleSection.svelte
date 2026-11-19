<script lang="ts">
  import { ChevronDown } from "lucide-svelte";
  import { slide } from "svelte/transition";
  const reducedMotion = typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  export let title = '';
  export let subtitle = '';
  export let storageKey = '';
  export let defaultOpen = false;

  let open = defaultOpen;
  if (storageKey && typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem(storageKey);
    if (saved !== null) open = saved === '1';
  }

  function toggle() {
    open = !open;
    if (storageKey && typeof localStorage !== 'undefined') {
      localStorage.setItem(storageKey, open ? '1' : '0');
    }
  }
</script>

<div class="settings-card">
  <button
    type="button"
    class="flex w-full items-center justify-between gap-3 text-left"
    aria-expanded={open}
    on:click={toggle}
  >
    <div class="min-w-0">
      <p class="settings-card-title">{title}</p>
      {#if subtitle}<p class="settings-card-desc">{subtitle}</p>{/if}
    </div>
    <ChevronDown size={16} class="shrink-0 text-slate-400 transition-transform duration-200 {open ? 'rotate-180' : ''}" />
  </button>
  {#if open}
    <div transition:slide={{duration:reducedMotion ? 0 : 160}} class="mt-3 settings-block">
      <slot />
    </div>
  {/if}
</div>
