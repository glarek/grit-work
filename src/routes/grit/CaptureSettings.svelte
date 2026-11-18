<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { estimateCapture, formatStorage, type CaptureSettings, type CaptureMonitor } from './captureSettings.ts';
  export let settings: CaptureSettings;
  export let monitors: CaptureMonitor[] = [];
  const dispatch = createEventDispatcher<{ saved: void }>();
  let draft = { ...settings };
  let previous = settings;
  let dirty = false;
  let saving = false;
  let message = '';
  let failed = false;
  let hours = 8;
  $: if (settings !== previous) { if (!dirty) draft = { ...settings }; previous = settings; }
  $: estimate = estimateCapture(draft, monitors, hours);
  $: fullDay = estimateCapture(draft, monitors, 24);
  async function save() {
    saving = true; message = ''; failed = false;
    try {
      await invoke('grit_save_capture_settings', { settings: draft });
      dirty = false;
      message = 'Insamlingen är uppdaterad.';
      dispatch('saved');
    } catch (error) { message = String(error); failed = true; }
    finally { saving = false; }
  }
</script>

<section class="grit-capture space-y-5 rounded-xl border border-slate-200 bg-white p-4 sm:p-5 dark:border-[#30363d] dark:bg-[#161b22]" aria-labelledby="capture-title">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <h2 id="capture-title" class="flex items-center gap-2 text-lg font-semibold text-slate-900 dark:text-[#e6edf3]"><svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 3h18v18H3zM3 16l5-5 4 4 3-3 6 6M15 7h.01"/></svg>Skärmbilder & lagring</h2>
    <span class="inline-flex items-center gap-1.5 text-xs text-slate-500 dark:text-[#8b949e]"><svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 4h18v13H3zM8 21h8m-4-4v4"/></svg>{monitors.length ? `${monitors.length} ${monitors.length === 1 ? 'skärm' : 'skärmar'} upptäckta` : 'Skärmar kunde inte identifieras'}</span>
  </div>
  <div class="grid gap-4 sm:grid-cols-2">
    <label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">Intervall (sekunder)
      <input aria-label="Skärmbildsintervall i sekunder" type="number" min="30" max="3600" step="1" bind:value={draft.interval_seconds} on:input={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:text-[#e6edf3]" />
      <span class="mt-1 block font-normal text-slate-500">30–3600 sekunder. Standard: 300.</span>
    </label>
<label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">Skärmar att samla in
      <select bind:value={draft.display_mode} on:change={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]">
        <option value="active_window">Skärmen med aktivt fönster</option><option value="all">Alla skärmar</option>
      </select>
      <span class="mt-1 block font-normal text-slate-500">Alla skärmar ger en gemensam bild.</span>
    </label>
  </div>
  <details class="border-y border-slate-100 py-3 dark:border-[#30363d]">
    <summary class="cursor-pointer py-1 text-sm font-semibold text-slate-700 dark:text-[#adbac7]">Bildinställningar <span class="ml-2 font-normal text-slate-500 dark:text-[#8b949e]">{draft.jpeg_quality} % JPEG</span></summary>
    <div class="mt-4 grid gap-4 sm:grid-cols-3">
      <label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">JPEG-kvalitet (%)
      <input type="number" min="30" max="95" bind:value={draft.jpeg_quality} on:input={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:text-[#e6edf3]" />
      <span class="mt-1 block font-normal text-slate-500">Högre kvalitet tar mer plats.</span>
    </label>
<label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">Bildstorlek
      <select bind:value={draft.width_mode} on:change={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]">
        <option value="auto">Automatisk</option><option value="fixed">Vald maxbredd</option>
      </select>
    </label>
<label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">Maxbredd (pixlar)
      <input type="number" min="800" max="16384" step="1" bind:value={draft.max_image_width} on:input={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:text-[#e6edf3]" />
      <span class="mt-1 block font-normal text-slate-500">Gäller hela bilden; automatik kan använda dubbel bredd.</span>
    </label>
    </div>
  </details>
  <section aria-labelledby="storage-estimate-title" class="space-y-3">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h3 id="storage-estimate-title" class="flex items-center gap-2 text-sm font-semibold text-slate-700 dark:text-[#adbac7]"><svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 4h16v16H4zM4 14h16M16 17h1"/></svg>Uppskattad lagring</h3>
      <label class="flex items-center gap-2 text-xs text-slate-500 dark:text-[#8b949e]">Beräkna för
        <input aria-label="Beräkna för (timmar per dag)" aria-describedby="estimate-hours-help" type="number" min="1" max="24" bind:value={hours} class="w-16 rounded-lg border border-slate-200 bg-transparent px-2 py-1.5 font-mono text-sm text-slate-900 dark:border-[#30363d] dark:text-[#e6edf3]" /> timmar/dag
      </label>
    </div>
    <p id="estimate-hours-help" class="text-xs text-slate-500 dark:text-[#8b949e]">Timmarna påverkar bara uppskattningen, inte insamlingen.</p>
    <dl class="grid gap-4 rounded-lg bg-slate-50 p-4 sm:grid-cols-3 dark:bg-[#0d1117]" aria-live="polite">
      <div><dt class="text-xs text-slate-500 dark:text-[#8b949e]">Bilder per dag</dt><dd class="mt-1 font-mono text-sm font-semibold text-slate-900 dark:text-[#e6edf3]">{estimate.count}</dd></div>
      <div><dt class="text-xs text-slate-500 dark:text-[#8b949e]">Lagring per dag</dt><dd class="mt-1 font-mono text-sm font-semibold text-slate-900 dark:text-[#e6edf3]">{formatStorage(estimate.low)}–{formatStorage(estimate.high)}</dd></div>
      <div><dt class="text-xs text-slate-500 dark:text-[#8b949e]">Långtidsarkiv, 30 dagar</dt><dd class="mt-1 font-mono text-sm font-semibold text-slate-900 dark:text-[#e6edf3]">{formatStorage(estimate.low * 30)}–{formatStorage(estimate.high * 30)}</dd></div>
    </dl>
    {#if estimate.assumedScreen}<p class="text-xs text-slate-500 dark:text-[#8b949e]">Ingen skärm kunde avläsas. Uppskattningen utgår från 1920 × 1080.</p>{/if}
    <p class="max-w-prose text-xs leading-relaxed text-slate-500 dark:text-[#8b949e]">Bilder rensas lokalt först efter verifierad arkivering. Om arkivet är otillgängligt växer den lokala lagringen.</p>
    <details class="text-xs text-slate-500 dark:text-[#8b949e]">
      <summary class="cursor-pointer py-1">Skärmar och beräkning</summary>
      {#if monitors.length}
        <ul class="mt-3 space-y-2" aria-label="Upptäckta skärmar">
          {#each monitors as monitor}
            <li class="flex flex-wrap items-center gap-x-3 gap-y-1"><svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 4h18v13H3zM8 21h8m-4-4v4"/></svg><span>{monitor.name}</span><span class="font-mono">{monitor.width} × {monitor.height}</span><span>{Math.round(monitor.scale_factor * 100)} % skalning</span></li>
          {/each}
        </ul>
      {/if}
      <p class="mt-3 max-w-prose leading-relaxed">Grovt intervall för JPEG-bilder; innehåll, upplösning och skalning påverkar storleken. Loggar tillkommer. Vid insamling dygnet runt: <span class="font-mono">{formatStorage(fullDay.low)}–{formatStorage(fullDay.high)}</span> per dygn.</p>
      <p class="mt-2 max-w-prose leading-relaxed">Kortare intervall ger fler bilder och längre rapportjobb. Insamling sker när den är aktiverad och skärmen är tillgänglig.</p>
    </details>
  </section>
  <div class="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 pt-4 dark:border-[#30363d]">
    <p role="status" class="text-sm {failed ? 'text-rose-600 dark:text-rose-400' : 'text-slate-500 dark:text-[#8b949e]'}">{message || (dirty ? 'Osparade ändringar' : 'Gäller nya skärmbilder')}</p>
    <button on:click={save} disabled={saving || !draft.interval_seconds || !draft.max_image_width || !draft.jpeg_quality} class="inline-flex items-center gap-2 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700 disabled:opacity-50"><svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 3h12l4 4v14H3V3h2Zm2 0v6h10V3M7 21v-8h10v8"/></svg>{saving ? 'Sparar...' : 'Spara insamling'}</button>
  </div>
</section>

<style>
  .grit-capture :is(button, input, select, summary):focus-visible {
    outline: 2px solid #6366f1;
    outline-offset: 3px;
  }
</style>
