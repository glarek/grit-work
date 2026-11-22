<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { slide } from 'svelte/transition';
  import { invoke } from '@tauri-apps/api/core';
  import { Camera, ChevronDown, HardDrive, Monitor, Save } from 'lucide-svelte';
  import UiSwitch from '$lib/components/ui/UiSwitch.svelte';
  import UiSelect from '$lib/components/ui/UiSelect.svelte';
  import HelpTip from '$lib/components/ui/HelpTip.svelte';
  import { estimateCapture, formatStorage, type CaptureSettings, type CaptureMonitor } from './captureSettings.ts';

  const displayModeOptions = [
    { value: 'active_window', label: 'Skärmen med aktivt fönster' },
    { value: 'all', label: 'Alla skärmar' },
  ];


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
  let showImageOptions = false;
  let showScreenDetails = false;

  $: if (settings !== previous) {
    if (!dirty) draft = { ...settings };
    previous = settings;
  }
  $: estimate = estimateCapture(draft, monitors, hours);
  $: fullDay = estimateCapture(draft, monitors, 24);

  async function save() {
    saving = true;
    message = '';
    failed = false;
    try {
      await invoke('grit_save_capture_settings', { settings: draft });
      dirty = false;
      message = 'Insamlingen är uppdaterad.';
      dispatch('saved');
    } catch (error) {
      message = String(error);
      failed = true;
    } finally {
      saving = false;
    }
  }
</script>

<section class="grit-capture grit-field-container space-y-5 rounded-xl border border-slate-200 bg-white p-5" aria-labelledby="capture-title">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <h2 id="capture-title" class="flex items-center gap-2 text-base font-semibold text-slate-900">
      <Camera class="h-4 w-4 text-slate-600" aria-hidden="true" />
      <span>Skärmbilder & lagring</span>
    </h2>
    <span class="inline-flex items-center gap-1.5 text-xs text-slate-500">
      <Monitor class="h-3.5 w-3.5" aria-hidden="true" />
      <span>{monitors.length ? `${monitors.length} ${monitors.length === 1 ? 'skärm' : 'skärmar'} upptäckta` : 'Skärmar kunde inte identifieras'}</span>
    </span>
  </div>

  <div class="grit-cols-2 grid gap-4">
    <div>
      <div class="mb-1.5 flex min-h-6 items-center">
        <label for="capture-interval" class="text-xs font-medium text-slate-700">
          Intervall (sekunder)
        </label>
      </div>
      <input
        id="capture-interval"
        aria-label="Skärmbildsintervall i sekunder"
        type="number"
        min="30"
        max="3600"
        step="1"
        bind:value={draft.interval_seconds}
        on:input={() => (dirty = true)}
        class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900"
      />
      <p class="mt-1 text-xs text-slate-500">30–3600 sekunder. Standard: 300.</p>
    </div>

    <div class="text-xs font-medium text-slate-700">
      <div class="mb-1.5 flex min-h-6 items-center gap-1.5">
        <span>Skärmar att samla in</span>
        <HelpTip label="Om skärmval" text="Alla skärmar slås samman till en gemensam bild." />
      </div>
      <UiSelect
        label="Skärmar att samla in"
        bind:value={draft.display_mode}
        options={displayModeOptions}
        on:change={() => (dirty = true)}
      />
    </div>
  </div>

  <div class="border-y border-slate-100 py-3">
    <button
      type="button"
      on:click={() => (showImageOptions = !showImageOptions)}
      aria-expanded={showImageOptions}
      class="flex w-full items-center justify-between py-1 text-left text-xs font-semibold text-slate-700"
    >
      <span>Bildinställningar <span class="ml-2 font-mono font-normal text-slate-500">{draft.jpeg_quality} % JPEG</span></span>
      <ChevronDown class="h-4 w-4 text-slate-400 transition-transform {showImageOptions ? 'rotate-180' : ''}" aria-hidden="true" />
    </button>

    {#if showImageOptions}
      <div transition:slide={{ duration: 180 }} class="grit-cols-3 mt-4 grid gap-4">
        <div>
          <div class="mb-1.5 flex min-h-6 items-center">
            <label for="capture-jpeg-quality" class="text-xs font-medium text-slate-700">
              JPEG-kvalitet (%)
            </label>
          </div>
          <input
            id="capture-jpeg-quality"
            type="number"
            min="30"
            max="95"
            bind:value={draft.jpeg_quality}
            on:input={() => (dirty = true)}
            class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900"
          />
          <p class="mt-1 text-xs text-slate-500">Högre kvalitet tar mer plats.</p>
        </div>

        <div class="space-y-2">
          <div class="flex min-h-6 items-center justify-between gap-3 text-xs font-medium text-slate-700">
            <span>Begränsa bildbredd</span>
            <UiSwitch checked={draft.max_image_width > 0} label="Begränsa bildbredd" on:change={(e) => { draft = { ...draft, max_image_width: e.detail ? 2560 : 0, width_mode: 'fixed' }; dirty = true; }} />
          </div>
          <p class="text-xs text-slate-500">{draft.max_image_width === 0 ? 'Originalupplösning. Rekommenderas för OCR och AI.' : 'Nedskalning kan göra liten text svår att läsa.'}</p>
        </div>
        {#if draft.max_image_width > 0}
        <div>
          <div class="mb-1.5 flex min-h-6 items-center">
            <label for="capture-max-width" class="text-xs font-medium text-slate-700">
              Maxbredd (pixlar)
            </label>
          </div>
          <input
            id="capture-max-width"
            type="number"
            min="800"
            max="16384"
            step="1"
            bind:value={draft.max_image_width}
            on:input={() => (dirty = true)}
            class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900"
          />
          <p class="mt-1 text-xs text-slate-500">Gäller hela bilden.</p>
        </div>
        {/if}
      </div>
    {/if}
  </div>

  <section aria-labelledby="storage-estimate-title" class="space-y-3">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h3 id="storage-estimate-title" class="flex items-center gap-2 text-sm font-semibold text-slate-800">
        <HardDrive class="h-4 w-4 text-slate-500" aria-hidden="true" />
        <span>Uppskattad lagring</span>
      </h3>
      <label class="flex items-center gap-2 text-xs text-slate-500">
        <span>Beräkna för</span>
        <input
          aria-label="Beräkna för (timmar per dag)"
          aria-describedby="estimate-hours-help"
          type="number"
          min="1"
          max="24"
          bind:value={hours}
          class="w-16 rounded-lg border border-slate-200 bg-white px-2 py-1 font-mono text-sm text-slate-900"
        />
        <span>timmar/dag</span>
      </label>
    </div>

    <p id="estimate-hours-help" class="text-xs text-slate-500">Timmarna påverkar bara uppskattningen, inte insamlingen.</p>

    <dl class="grid gap-4 rounded-xl bg-slate-50 p-4 sm:grid-cols-3" aria-live="polite">
      <div>
        <dt class="text-xs text-slate-500">Bilder per dag</dt>
        <dd class="mt-1 font-mono text-sm font-semibold text-slate-900">{estimate.count}</dd>
      </div>
      <div>
        <dt class="text-xs text-slate-500">Lagring per dag</dt>
        <dd class="mt-1 font-mono text-sm font-semibold text-slate-900">{formatStorage(estimate.low)}–{formatStorage(estimate.high)}</dd>
      </div>
      <div>
        <dt class="text-xs text-slate-500">Långtidsarkiv, 30 dagar</dt>
        <dd class="mt-1 font-mono text-sm font-semibold text-slate-900">{formatStorage(estimate.low * 30)}–{formatStorage(estimate.high * 30)}</dd>
      </div>
    </dl>

    {#if estimate.assumedScreen}
      <p class="text-xs text-slate-500">Ingen skärm kunde avläsas. Uppskattningen utgår från 1920 × 1080.</p>
    {/if}

    <div>
      <button
        type="button"
        on:click={() => (showScreenDetails = !showScreenDetails)}
        aria-expanded={showScreenDetails}
        class="inline-flex items-center gap-1.5 py-1 text-xs font-medium text-slate-600 hover:text-slate-900"
      >
        <span>Skärmar och beräkning</span>
        <ChevronDown class="h-3.5 w-3.5 transition-transform {showScreenDetails ? 'rotate-180' : ''}" aria-hidden="true" />
      </button>

      {#if showScreenDetails}
        <div transition:slide={{ duration: 180 }} class="mt-2 space-y-2 text-xs text-slate-500">
          {#if monitors.length}
            <ul class="space-y-1.5" aria-label="Upptäckta skärmar">
              {#each monitors as monitor}
                <li class="flex flex-wrap items-center gap-x-3 gap-y-1">
                  <Monitor class="h-3.5 w-3.5 text-slate-400" aria-hidden="true" />
                  <span>{monitor.name}</span>
                  <span class="font-mono">{monitor.width} × {monitor.height}</span>
                  <span>{Math.round(monitor.scale_factor * 100)} % skalning</span>
                </li>
              {/each}
            </ul>
          {/if}
          <p class="max-w-prose leading-relaxed">
            Grovt intervall för JPEG-bilder; innehåll, upplösning och skalning påverkar storleken. Vid insamling dygnet runt:
            <span class="font-mono">{formatStorage(fullDay.low)}–{formatStorage(fullDay.high)}</span> per dygn.
          </p>
        </div>
      {/if}
    </div>
  </section>

  <div class="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 pt-4">
    <p role="status" class="text-xs {failed ? 'text-rose-600' : 'text-slate-500'}">
      {message || (dirty ? 'Osparade ändringar' : 'Gäller nya skärmbilder')}
    </p>
    <button
      type="button"
      on:click={save}
      disabled={saving || !draft.interval_seconds || !Number.isFinite(draft.max_image_width) || !draft.jpeg_quality}
      class="inline-flex items-center gap-1.5 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
    >
      <Save class="h-3.5 w-3.5" aria-hidden="true" />
      <span>{saving ? 'Sparar...' : 'Spara insamling'}</span>
    </button>
  </div>
</section>

<style>
  .grit-field-container {
    container-type: inline-size;
  }

  .grit-cols-2,
  .grit-cols-3 {
    grid-template-columns: minmax(0, 1fr);
  }

  @container (min-width: 580px) {
    .grit-cols-2 {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @container (min-width: 600px) {
    .grit-cols-3 {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  .grit-capture :is(button, input, select, summary):focus-visible {
    outline: 2px solid #4f46e5;
    outline-offset: 2px;
  }

  @media (prefers-reduced-motion: reduce) {
    .grit-capture * {
      transition-duration: 0.01ms !important;
      animation-duration: 0.01ms !important;
    }
  }
</style>
