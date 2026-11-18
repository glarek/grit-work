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
      message = 'Insamlingen är uppdaterad. Nästa skärmbild följer det nya intervallet.';
      dispatch('saved');
    } catch (error) { message = String(error); failed = true; }
    finally { saving = false; }
  }
</script>

<section class="grit-capture space-y-5 rounded-xl border border-slate-200 bg-white p-5 dark:border-[#30363d] dark:bg-[#161b22]" aria-labelledby="capture-title">
  <div class="flex flex-wrap items-start justify-between gap-4">
    <div>
      <h2 id="capture-title" class="text-lg font-semibold text-slate-900 dark:text-[#e6edf3]">Skärmbilder & lagring</h2>
      <p class="mt-1 text-sm text-slate-500 dark:text-[#8b949e]">Välj hur mycket av arbetsdagen som ska sparas.</p>
    </div>
    <span class="rounded-full bg-slate-100 px-3 py-1 text-xs text-slate-600 dark:bg-[#21262d] dark:text-[#c9d1d9]">{monitors.length ? `${monitors.length} ${monitors.length === 1 ? 'skärm' : 'skärmar'} upptäckta` : 'Skärmar kunde inte identifieras'}</span>
  </div>
  <div class="grid gap-4 sm:grid-cols-3">
    <label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">Intervall (sekunder)
      <input aria-label="Skärmbildsintervall i sekunder" type="number" min="30" max="3600" step="1" bind:value={draft.interval_seconds} on:input={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:text-[#e6edf3]" />
      <span class="mt-1 block font-normal text-slate-500">30 sekunder–60 minuter. Standard: 5 minuter.</span>
    </label>
    <label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">Skärmar att samla in
      <select bind:value={draft.display_mode} on:change={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]">
        <option value="active_window">Skärmen med aktivt fönster</option><option value="all">Alla skärmar</option>
      </select>
      <span class="mt-1 block font-normal text-slate-500">Alla skärmar sparas tillsammans i en bild.</span>
    </label>
    <label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">JPEG-kvalitet (%)
      <input type="number" min="30" max="95" bind:value={draft.jpeg_quality} on:input={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:text-[#e6edf3]" />
      <span class="mt-1 block font-normal text-slate-500">Högre kvalitet tar mer plats.</span>
    </label>
  </div>
  <div class="grid gap-4 sm:grid-cols-3">
    <label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">Bildstorlek
      <select bind:value={draft.width_mode} on:change={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]">
        <option value="auto">Automatisk</option><option value="fixed">Vald maxbredd</option>
      </select>
    </label>
    <label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">Maxbredd (pixlar)
      <input type="number" min="800" max="16384" step="1" bind:value={draft.max_image_width} on:input={() => dirty = true} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:text-[#e6edf3]" />
      <span class="mt-1 block font-normal text-slate-500">Gäller hela bilden. Automatik kan använda upp till dubbel bredd.</span>
    </label>
    <label class="text-xs font-semibold text-slate-600 dark:text-[#c9d1d9]">Beräkna för (timmar per dag)
      <input type="number" min="1" max="24" bind:value={hours} class="mt-1.5 w-full rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:text-[#e6edf3]" />
      <span class="mt-1 block font-normal text-slate-500">Påverkar bara uppskattningen, inte inspelningen.</span>
    </label>
  </div>
  {#if monitors.length}
    <div class="flex flex-wrap gap-2">
      {#each monitors as monitor}
        <span class="rounded-lg border border-slate-200 px-3 py-1.5 text-xs text-slate-600 dark:border-[#30363d] dark:text-[#8b949e]">{monitor.name} <span class="font-mono">{monitor.width} × {monitor.height}</span> · {Math.round(monitor.scale_factor * 100)} %</span>
      {/each}
    </div>
  {/if}
  <div class="rounded-lg bg-slate-50 p-4 dark:bg-[#0d1117]" aria-live="polite">
    <div class="grid gap-4 sm:grid-cols-3">
      <div><p class="text-xs text-slate-500">Bilder per dag</p><p class="grit-metric font-semibold text-slate-900 dark:text-[#e6edf3]">{estimate.count}</p></div>
      <div><p class="text-xs text-slate-500">Skärmbilder per dag</p><p class="font-mono text-sm font-semibold text-slate-900 dark:text-[#e6edf3]">{formatStorage(estimate.low)}–{formatStorage(estimate.high)}</p></div>
      <div><p class="text-xs text-slate-500">Långtidsarkiv, 30 dagar</p><p class="font-mono text-sm font-semibold text-slate-900 dark:text-[#e6edf3]">{formatStorage(estimate.low * 30)}–{formatStorage(estimate.high * 30)}</p></div>
    </div>
    <p class="mt-3 text-xs leading-relaxed text-slate-500 dark:text-[#8b949e]">Grovt planeringsintervall för JPEG-bilder. Innehåll, upplösning och skalning påverkar storleken; loggar tillkommer. {estimate.assumedScreen ? 'Ingen skärm kunde avläsas; exemplet utgår från 1920 × 1080.' : ''} Vid insamling dygnet runt: {formatStorage(fullDay.low)}–{formatStorage(fullDay.high)} per dygn.</p>
    <p class="mt-2 text-xs text-slate-500 dark:text-[#8b949e]">Lokala bilder rensas först efter verifierad arkivering. Om arkivet är otillgängligt fortsätter den lokala lagringen att växa. Kortare intervall ger fler bilder och längre rapportjobb. Inspelning sker när den är aktiverad och skärmen tillgänglig.</p>
  </div>
  <div class="flex flex-wrap items-center justify-between gap-3">
    <p role="status" class="text-sm {failed ? 'text-rose-600' : 'text-slate-500 dark:text-[#8b949e]'}">{message || (dirty ? 'Du har osparade ändringar.' : 'Inställningarna gäller nya skärmbilder.')}</p>
    <button on:click={save} disabled={saving || !draft.interval_seconds || !draft.max_image_width || !draft.jpeg_quality} class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700 disabled:opacity-50">{saving ? 'Sparar...' : 'Spara insamling'}</button>
  </div>
</section>
