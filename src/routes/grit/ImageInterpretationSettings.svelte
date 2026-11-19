<script lang="ts">
  import { createEventDispatcher, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Check, Play, RefreshCw, ScanText, X } from 'lucide-svelte';
  import UiSelect from '../../lib/components/ui/UiSelect.svelte';
  import HelpTip from '../../lib/components/ui/HelpTip.svelte';
  import { workDate } from './workflow.ts';
  import {
    getDefaultModelForProvider,
    PROVIDER_LABELS,
    type GritImageInterpretationMode,
    type GritImageInterpretationSettings,
    type GritImageInterpretationStatus,
    type GritProvider,
  } from './gritTypes';

  export let settings: GritImageInterpretationSettings;
  export let aiConsent = false;
  export let dirty = false;
  export let saving = false;

  const dispatch = createEventDispatcher<{
    save: void;
  }>();

  let status: GritImageInterpretationStatus | null = null;
  let loadingStatus = false;
  let runningRequest = false;
  let canceling = false;
  let statusError = '';
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let activePollMs = 0;
  let loadedDate = '';

  const modeOptions: { value: GritImageInterpretationMode; label: string }[] = [
    { value: 'off', label: 'Av (ingen bildtolkning)' },
    { value: 'ocr', label: 'Lokal OCR (Windows)' },
    { value: 'ai', label: 'AI-bildtolkning' },
  ];

  const providerOptions: { value: GritProvider; label: string }[] = [
    { value: 'codex', label: `${PROVIDER_LABELS.codex.name} (${getDefaultModelForProvider('codex')})` },
    { value: 'grok', label: `${PROVIDER_LABELS.grok.name} (${getDefaultModelForProvider('grok')})` },
    { value: 'agy', label: `${PROVIDER_LABELS.agy.name} (${getDefaultModelForProvider('agy')})` },
  ];

  $: activeDate = $workDate;
  $: isRunning = Boolean(runningRequest || status?.running);
  $: needsConsentForRun = settings.mode === 'ai' && !aiConsent;
  $: runDisabled = dirty || saving || needsConsentForRun;
  $: desiredPollMs = isRunning ? 2500 : 10000;

  $: if (activeDate && activeDate !== loadedDate) {
    loadedDate = activeDate;
    status = null;
    statusError = '';
    void refreshStatus(activeDate);
  }

  $: if (activeDate && desiredPollMs !== activePollMs) {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
    }
    activePollMs = desiredPollMs;
    pollTimer = setInterval(() => {
      void refreshStatus(activeDate);
    }, desiredPollMs);
  }

  onDestroy(() => {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  });

  function handleModeChange(nextMode: string) {
    if (nextMode !== 'off' && nextMode !== 'ocr' && nextMode !== 'ai') return;
    settings = { ...settings, mode: nextMode };
  }

  function handleProviderChange(nextProvider: string) {
    if (nextProvider !== 'codex' && nextProvider !== 'grok' && nextProvider !== 'agy') return;
    settings = {
      ...settings,
      provider: nextProvider,
      model: getDefaultModelForProvider(nextProvider),
    };
  }

  async function refreshStatus(requestDate: string) {
    if (!requestDate) return;
    loadingStatus = true;
    try {
      const next = await invoke<GritImageInterpretationStatus>('grit_interpretation_status', {
        date: requestDate,
      });
      if (requestDate === activeDate) {
        status = next;
        statusError = '';
      }
    } catch (err) {
      if (requestDate === activeDate) {
        statusError = err instanceof Error ? err.message : String(err);
      }
    } finally {
      if (requestDate === activeDate) {
        loadingStatus = false;
      }
    }
  }

  async function handleRunInterpretation() {
    const requestDate = activeDate;
    if (!requestDate || runDisabled) return;
    runningRequest = true;
    statusError = '';
    try {
      const next = await invoke<GritImageInterpretationStatus>('grit_interpret_images', {
        date: requestDate,
        retryFailed: true,
      });
      if (requestDate === activeDate) {
        status = next;
      }
    } catch (err) {
      if (requestDate === activeDate) {
        statusError = err instanceof Error ? err.message : String(err);
      }
    } finally {
      runningRequest = false;
    }
  }

  async function handleCancelInterpretation() {
    const requestDate = activeDate;
    canceling = true;
    try {
      await invoke('grit_cancel');
      await refreshStatus(requestDate);
    } catch (err) {
      if (requestDate === activeDate) {
        statusError = err instanceof Error ? err.message : String(err);
      }
    } finally {
      canceling = false;
    }
  }
</script>

<section class="space-y-4 rounded-xl border border-slate-200 bg-white p-5">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div class="space-y-1">
      <div class="flex items-center gap-2">
        <ScanText class="h-4 w-4 text-slate-600" aria-hidden="true" />
        <h2 class="text-base font-semibold text-slate-900">Bildtolkning</h2>
        <HelpTip
          label="Om bildtolkning"
          text="Eget intervall separat från skärmdumpstagning. Läser redan tagna skärmbilder och sparar strukturerad data i image-interpretations/YYYY-MM-DD.json. Av skriver tomt innehåll, Lokal OCR körs lokalt på Windows via Rust och AI skickar bilder till vald leverantör (kräver AI-samtycke)."
        />
      </div>
      <p class="text-xs text-slate-500">
        {settings.mode === 'off'
          ? 'Avstängt: inget bildinnehåll tolkas eller skickas.'
          : settings.mode === 'ocr'
            ? 'Tolkar utvalda bilder lokalt. Inget bildinnehåll skickas.'
            : 'AI tolkar utvalda skärmbilder till strukturerad JSON. Tidrapportens AI använder dessa tolkningar.'}
      </p>
    </div>
  </div>

  <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
    <div>
      <div class="mb-1 flex min-h-6 items-center gap-1.5">
        <label for="grit-image-interp-mode" class="text-xs font-medium text-slate-700">
          Tolkningsläge
        </label>
      </div>
      <UiSelect
        id="grit-image-interp-mode"
        label="Tolkningsläge"
        value={settings.mode}
        options={modeOptions}
        on:change={(e) => handleModeChange(e.detail)}
      />
    </div>

    <div>
      <div class="mb-1 flex min-h-6 items-center gap-1.5">
        <label for="grit-image-interp-interval" class="text-xs font-medium text-slate-700">
          Tolkningsintervall (sekunder)
        </label>
        <HelpTip
          label="Om tolkningsintervall"
          text="Hur ofta en tagen skärmbild tolkas (30–3600 sek, standard 300 sek). Oberoende av hur ofta skärmbilder tas."
        />
      </div>
      <input
        id="grit-image-interp-interval"
        type="number"
        min="30"
        max="3600"
        step="30"
        disabled={settings.mode === 'off'}
        bind:value={settings.interval_seconds}
        class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900 disabled:bg-slate-50 disabled:text-slate-400"
      />
    </div>
  </div>

  {#if settings.mode === 'ai'}
    <div class="grid grid-cols-1 gap-3 rounded-xl border border-slate-200 bg-slate-50/70 p-4 sm:grid-cols-2">
      <div>
        <div class="mb-1 flex min-h-6 items-center">
          <label for="grit-image-interp-provider" class="text-xs font-medium text-slate-700">
            AI-leverantör för bildtolkning
          </label>
        </div>
        <UiSelect
          id="grit-image-interp-provider"
          label="AI-leverantör för bildtolkning"
          value={settings.provider}
          options={providerOptions}
          on:change={(e) => handleProviderChange(e.detail)}
        />
      </div>

      <div>
        <div class="mb-1 flex min-h-6 items-center">
          <label for="grit-image-interp-model" class="text-xs font-medium text-slate-700">
            Modell
          </label>
        </div>
        <input
          id="grit-image-interp-model"
          type="text"
          bind:value={settings.model}
          placeholder={getDefaultModelForProvider(settings.provider)}
          class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900"
        />
        <p class="mt-1 text-[11px] text-slate-500">
          Rekommenderad: <code class="font-mono">{getDefaultModelForProvider(settings.provider)}</code>
        </p>
      </div>

      {#if !aiConsent}
        <p class="sm:col-span-2 text-xs font-medium text-amber-700">
          Kräver sparat AI-samtycke under fliken AI-rapportering innan AI-bildtolkning körs.
        </p>
      {/if}
    </div>
  {/if}

  <div class="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 pt-3">
    <div class="flex flex-wrap items-center gap-2 text-xs text-slate-600">
      <span class="font-mono font-semibold text-slate-800">{status?.date ?? activeDate}</span>
      {#if status}
        <span>
          · <span class="font-mono">{status.completed}</span> klara,
          <span class="font-mono">{status.pending}</span> väntar,
          <span class="font-mono">{status.failed}</span> fel,
          <span class="font-mono">{status.disabled}</span> av,
          <span class="font-mono">{status.skipped}</span> hoppade över
        </span>
        {#if isRunning}
          <span class="inline-flex items-center gap-1 rounded-full bg-indigo-50 px-2 py-0.5 text-[11px] font-medium text-indigo-700">
            Tolkar…
          </span>
        {/if}
      {:else if loadingStatus}
        <span>Läser status…</span>
      {/if}
    </div>

    <div class="flex flex-wrap items-center gap-2">
      {#if dirty}
        <span class="text-xs font-medium text-amber-700">Spara ändringarna först</span>
      {/if}

      {#if isRunning}
        <button
          type="button"
          on:click={handleCancelInterpretation}
          disabled={canceling}
          class="inline-flex h-10 items-center gap-1.5 rounded-xl border border-rose-200 bg-rose-50 px-3 text-xs font-semibold text-rose-700 transition hover:bg-rose-100 disabled:opacity-50"
        >
          <X class="h-3.5 w-3.5" aria-hidden="true" />
          <span>{canceling ? 'Avbryter...' : 'Avbryt'}</span>
        </button>
      {:else}
        <button
          type="button"
          on:click={handleRunInterpretation}
          disabled={runDisabled}
          class="inline-flex h-10 items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-50"
        >
          {#if settings.mode === 'off'}
            <Play class="h-3.5 w-3.5" aria-hidden="true" />
            <span>Skapa tomt underlag</span>
          {:else if (status?.failed ?? 0) > 0}
            <RefreshCw class="h-3.5 w-3.5" aria-hidden="true" />
            <span>Försök igen</span>
          {:else}
            <Play class="h-3.5 w-3.5" aria-hidden="true" />
            <span>Tolka nu</span>
          {/if}
        </button>
      {/if}

      <button
        type="button"
        on:click={() => dispatch('save')}
        disabled={saving}
        class="inline-flex h-10 items-center gap-1.5 rounded-xl bg-indigo-600 px-4 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
      >
        <Check class="h-3.5 w-3.5" aria-hidden="true" />
        <span>{saving ? 'Sparar...' : 'Spara bildtolkning'}</span>
      </button>
    </div>
  </div>

  {#if statusError || status?.last_error}
    <p class="text-xs text-rose-700">{statusError || status?.last_error}</p>
  {/if}
</section>
