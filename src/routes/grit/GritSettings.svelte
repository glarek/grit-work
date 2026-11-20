<script lang="ts">
  import UiCheckbox from "$lib/components/ui/UiCheckbox.svelte";
  import { createEventDispatcher, onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import {
    AlertCircle,
    Building2,
    Check,
    CheckCircle2,
    ChevronDown,
    FolderOpen,
    KeyRound,
    RefreshCw,
    ShieldCheck,
    Sparkles,
  } from 'lucide-svelte';
  import AiConnectionTest from './AiConnectionTest.svelte';
  import CaptureSettings from './CaptureSettings.svelte';
  import ImageInterpretationSettings from './ImageInterpretationSettings.svelte';
  import UiSelect from '$lib/components/ui/UiSelect.svelte';
  import HelpTip from '$lib/components/ui/HelpTip.svelte';
  import { notifyGritChanged } from './workflow.ts';
  import {
    applyProviderChange,
    createDefaultSettings,
    normalizeGritStatus,
    PROVIDER_LABELS,
    type GritProvider,
    type GritSettings as GritSettingsModel,
    type GritStatus,
  } from './gritTypes.ts';

  const resourceLabels: Record<string,string> = {
    'Core/Projects':'Projekt', 'Admin/Users':'Användare', 'Admin/Activities':'Aktiviteter',
    'Admin/TimeCodes':'Tidskoder', 'Admin/Articles':'Artiklar', 'Core/TimeReports':'Tidrapporter',
  };
  type AuthMode = 'oauth' | 'static';
  type GritSettingsSection = 'all' | 'ai' | 'spiris' | 'capture';

  interface ConnectionReadCheck {
    resource: string;
    ok: boolean;
    tested: boolean;
    message: string;
    count?: number;
  }

  interface ConnectionTestResult {
    checked_at: string | number;
    authentication: { ok: boolean; tested: boolean; message: string };
    reads: ConnectionReadCheck[];
    submission_permission: string;
  }

  const providerOptions = [
    { value: 'codex', label: 'Codex CLI (OpenAI)' },
    { value: 'grok', label: 'Grok CLI (xAI)' },
    { value: 'agy', label: 'Antigravity CLI (Google Gemini)' },
  ];

  export let section: GritSettingsSection = 'all';
  export let initialStatus: GritStatus | null = null;

  const dispatch = createEventDispatcher<{
    statusUpdated: GritStatus;
    saved: void;
    connectionState: {
      aiReady: boolean;
      credentialSaved: boolean;
      connectionVerified: boolean;
    };
  }>();

  function cloneSettings(s: GritSettingsModel): GritSettingsModel {
    return {
      ...s,
      image_interpretation: { ...s.image_interpretation },
    };
  }

  let status: GritStatus | null = initialStatus;
  let settingsForm: GritSettingsModel = initialStatus
    ? cloneSettings(initialStatus.settings)
    : createDefaultSettings();
  let savedSettings: GritSettingsModel = cloneSettings(settingsForm);

  function emitConnectionState(): void {
    dispatch('connectionState', {
      aiReady: Boolean(status?.settings.ai_consent && status?.settings.provider),
      credentialSaved: Boolean(status?.credential_saved),
      connectionVerified,
    });
  }

  function invalidateVerification(): void {
    if (connectionVerified || verifiedUserId !== null || connectionResult !== null) {
      connectionVerified = false;
      verifiedUserId = null;
      connectionResult = null;
    }
    emitConnectionState();
  }

  function adoptStatus(next: GritStatus, force = false): void {
    const clean = JSON.stringify(settingsForm) === JSON.stringify(savedSettings);
    status = next;
    if (clean || force) {
      settingsForm = cloneSettings(next.settings);
      savedSettings = cloneSettings(next.settings);
    }
    if (!next.credential_saved || (verifiedUserId !== null && next.settings.user_id !== verifiedUserId)) {
      connectionVerified = false;
      verifiedUserId = null;
      connectionResult = null;
    }
    emitConnectionState();
  }

  let loading = !initialStatus;
  let savingSettings = false;
  let savingCredentials = false;
  let syncingSpiris = false;
  let testingConnection = false;
  let openingLocalDir = false;
  let connectionVerified = false;
  let verifiedUserId: number | null = null;
  let connectionResult: ConnectionTestResult | null = null;
  let errorMessage: string | null = null;
  let noticeMessage: string | null = null;
  let showAdvancedAi = false;

  let authMode: AuthMode = 'oauth';
  let applicationId = '';
  let secret = '';
  let staticToken = '';

  let lastInitialRef: GritStatus | null = initialStatus;
  $: if (initialStatus && initialStatus !== lastInitialRef) {
    lastInitialRef = initialStatus;
    adoptStatus(initialStatus);
    loading = false;
  }

  $: providerInfo = PROVIDER_LABELS[settingsForm.provider];
  $: imageProviderInfo = PROVIDER_LABELS[settingsForm.image_interpretation.provider];
  $: settingsDirty = JSON.stringify(settingsForm) !== JSON.stringify(savedSettings);
  $: userOptions = [
    { value: '0', label: 'Välj användare...' },
    ...(status?.users ?? []).map((u) => ({ value: String(u.id), label: u.name })),
  ];
  $: showAi = section === 'all' || section === 'ai';
  $: showSpiris = section === 'all' || section === 'spiris';
  $: showCapture = section === 'all' || section === 'capture';

  function formatError(err: unknown): string {
    if (typeof err === 'string' && err.trim()) return err;
    if (err instanceof Error && err.message.trim()) return err.message;
    return 'Ett oväntat fel uppstod.';
  }

  function emitSaved(): void {
    dispatch('saved');
    notifyGritChanged();
  }

  async function handleCaptureSaved(): Promise<void> {
    await refreshStatus();
    emitSaved();
  }

  export async function refreshStatus(force = false): Promise<void> {
    errorMessage = null;
    try {
      const rawStatus = await invoke<unknown>('grit_status');
      const nextStatus = normalizeGritStatus(rawStatus);
      adoptStatus(nextStatus, force);
      dispatch('statusUpdated', nextStatus);
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      loading = false;
    }
  }

  async function handleSyncSpiris(): Promise<void> {
    errorMessage = null;
    noticeMessage = null;
    syncingSpiris = true;
    try {
      const rawStatus = await invoke<unknown>('grit_sync_spiris');
      const nextStatus = normalizeGritStatus(rawStatus);
      adoptStatus(nextStatus);
      dispatch('statusUpdated', nextStatus);
      noticeMessage = `Spiris synkroniserat (${nextStatus.projects.length} projekt). Kör Testa anslutning för full läskontroll.`;
      emitSaved();
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      syncingSpiris = false;
    }
  }

  async function handleTestConnection(): Promise<void> {
    errorMessage = null;
    noticeMessage = null;
    connectionResult = null;
    connectionVerified = false;
    verifiedUserId = null;
    testingConnection = true;
    try {
      const raw = await invoke<unknown>('grit_test_connection');
      if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
        throw new Error('Ogiltigt svar från anslutningskontrollen.');
      }
      const rec = raw as Record<string, unknown>;
      const authRaw = (rec.authentication && typeof rec.authentication === 'object'
        ? rec.authentication
        : null) as Record<string, unknown> | null;
      const readsRaw = Array.isArray(rec.reads) ? rec.reads : [];
      if (!authRaw || readsRaw.length === 0) {
        throw new Error('Anslutningskontrollen saknar autentiserings- eller läsresultat.');
      }

      const parsed: ConnectionTestResult = {
        checked_at: (typeof rec.checked_at === 'string' || typeof rec.checked_at === 'number')
          ? rec.checked_at
          : '',
        authentication: {
          ok: authRaw.ok === true,
          tested: authRaw.tested === true,
          message: typeof authRaw.message === 'string' ? authRaw.message : '',
        },
        reads: readsRaw
          .filter((r): r is Record<string, unknown> => Boolean(r && typeof r === 'object'))
          .map((r) => ({
            resource: typeof r.resource === 'string' ? r.resource : 'resurs',
            ok: r.ok === true,
            tested: r.tested === true,
            message: typeof r.message === 'string' ? r.message : '',
            count: typeof r.count === 'number' ? r.count : undefined,
          })),
        submission_permission:
          typeof rec.submission_permission === 'string'
            ? rec.submission_permission
            : 'not_tested',
      };

      connectionResult = parsed;
      const allSixReadsOk =
        parsed.reads.length >= 6 && parsed.reads.every((r) => r.ok && r.tested);
      connectionVerified =
        parsed.authentication.ok && parsed.authentication.tested && allSixReadsOk;

      if (connectionVerified) {
        verifiedUserId = settingsForm.user_id;
        noticeMessage =
          'Läskontroll mot Spiris lyckades (6/6 resurser). Skrivbehörighet för tidrapporter är inte testad.';
      } else {
        verifiedUserId = null;
        errorMessage = 'Anslutningskontrollen kunde inte verifiera alla 6 läskontroller.';
      }
    } catch (err) {
      connectionVerified = false;
      verifiedUserId = null;
      errorMessage = formatError(err);
    } finally {
      emitConnectionState();
      testingConnection = false;
      notifyGritChanged();
    }
  }

  async function handlePickArchiveDir(): Promise<void> {
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        defaultPath: settingsForm.archive_dir || status?.local_dir || undefined,
      });
      if (typeof picked === 'string' && picked.trim()) {
        settingsForm = { ...settingsForm, archive_dir: picked };
      }
    } catch (err) {
      errorMessage = formatError(err);
    }
  }

  async function handleOpenLocalDir(): Promise<void> {
    errorMessage = null;
    openingLocalDir = true;
    try {
      await invoke('open_screenshots_dir');
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      openingLocalDir = false;
    }
  }

  function handleProviderSelect(nextProvider: GritProvider): void {
    settingsForm = applyProviderChange(settingsForm, nextProvider);
  }

  async function handleSaveSettings(): Promise<void> {
    errorMessage = null;
    noticeMessage = null;
    savingSettings = true;
    try {
      await invoke('grit_save_settings', { settings: settingsForm });
      await refreshStatus(true);
      noticeMessage = 'Inställningarna har sparats.';
      emitSaved();
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      savingSettings = false;
    }
  }

  async function handleSaveCredentials(): Promise<void> {
    errorMessage = null;
    noticeMessage = null;
    connectionResult = null;
    savingCredentials = true;
    try {
      const payload =
        authMode === 'oauth'
          ? {
              applicationId: applicationId.trim(),
              secret: secret.trim(),
              staticToken: '',
            }
          : {
              applicationId: '',
              secret: '',
              staticToken: staticToken.trim(),
            };

      await invoke('grit_save_credentials', payload);
      applicationId = '';
      secret = '';
      staticToken = '';
      invalidateVerification();
      await refreshStatus();
      noticeMessage = 'Spiris-uppgifterna har sparats i datorns nyckellager. Testa anslutningen för att verifiera.';
      emitSaved();
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      savingCredentials = false;
    }
  }

  onMount(() => {
    void refreshStatus();
    const onUserChanged = (event: Event) => {
      const userId = (event as CustomEvent<{ userId: number }>).detail.userId;
      // Preserve unsaved AI/capture edits while adopting the explicit profile selection.
      settingsForm = { ...settingsForm, user_id: userId };
      savedSettings = { ...savedSettings, user_id: userId };
      invalidateVerification();
      void refreshStatus();
    };
    window.addEventListener('grit-user-changed', onUserChanged);
    return () => window.removeEventListener('grit-user-changed', onUserChanged);
  });
</script>

<div class="grit-settings space-y-5">
  {#if errorMessage}
    <div role="alert" class="flex items-start gap-2.5 rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-800">
      <AlertCircle class="mt-0.5 h-4 w-4 shrink-0 text-rose-600" aria-hidden="true" />
      <div>
        <div class="font-semibold">Fel</div>
        <p class="mt-0.5 whitespace-pre-wrap text-xs">{errorMessage}</p>
      </div>
    </div>
  {/if}

  {#if noticeMessage}
    <div role="status" class="flex items-center gap-2 rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-xs font-medium text-emerald-800">
      <CheckCircle2 class="h-4 w-4 shrink-0 text-emerald-600" aria-hidden="true" />
      <span>{noticeMessage}</span>
    </div>
  {/if}

  {#if loading && !status}
    <div class="rounded-xl border border-slate-200 bg-white p-8 text-center text-sm text-slate-500">
      Laddar GRIT Work-inställningar...
    </div>
  {:else if status}
    {#if showAi}
      <section class="space-y-4 rounded-xl border border-slate-200 bg-white p-5">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="flex items-center gap-2">
            <Sparkles class="h-4 w-4 text-slate-600" aria-hidden="true" />
            <div>
              <h2 class="text-base font-semibold text-slate-900">AI & rapportering</h2>
              <p class="text-xs text-slate-500">Välj AI-tjänst och samtycke för rapportutkast.</p>
            </div>
          </div>
          <span class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium {settingsForm.ai_consent
            ? 'bg-emerald-50 text-emerald-800'
            : 'bg-amber-50 text-amber-800'}">
            {#if settingsForm.ai_consent}
              <ShieldCheck class="h-3.5 w-3.5 text-emerald-600" aria-hidden="true" />
              <span>Samtycke aktivt</span>
            {:else}
              <AlertCircle class="h-3.5 w-3.5 text-amber-600" aria-hidden="true" />
              <span>Samtycke saknas</span>
            {/if}
          </span>
        </div>

        <div class="space-y-3">
          <AiConnectionTest target="report" configKey={settingsForm.provider + ':' + settingsForm.model} disabled={settingsDirty || savingSettings} />
          <div class="text-xs font-medium text-slate-700">
            <div class="mb-1">AI-tjänst</div>
            <UiSelect
              label="AI-tjänst"
              value={settingsForm.provider}
              options={providerOptions}
              on:change={(e) => {
                const val = e.detail;
                if (val === 'codex' || val === 'grok' || val === 'agy') {
                  handleProviderSelect(val);
                }
              }}
            />
          </div>

          <label for="gritsettings-settingsForm-ai_consent" class="flex cursor-pointer items-start gap-2.5 rounded-xl border p-3 transition {settingsForm.ai_consent
            ? 'border-emerald-200 bg-emerald-50/40'
            : 'border-amber-200 bg-amber-50/40'}">
            <UiCheckbox id="gritsettings-settingsForm-ai_consent" bind:checked={settingsForm.ai_consent} label="Samtycke till valda AI-tjänster" />
            <div>
              <div class="flex items-center gap-1.5 text-xs font-semibold text-slate-900">
                <span>Samtycke till valda AI-tjänster</span>
                <HelpTip
                  label="Om AI-samtycke"
                  text={`Tidrapportens AI skickar aktivitetsloggar och sparade bildtolkningar till ${providerInfo.name} (${providerInfo.vendor}). Endast när AI-bildtolkning är vald under Insamling & lagring skickas även skärmbilder till ${imageProviderInfo.name} (${imageProviderInfo.vendor}).`}
                />
              </div>
              <p class="mt-0.5 text-xs text-slate-600">
                Krävs för tidrapportens AI ({providerInfo.name}) och för valfri AI-bildtolkning ({imageProviderInfo.name}).
              </p>
            </div>
          </label>

          <label for="gritsettings-settingsForm-use_report_history" class="flex cursor-pointer items-start gap-2.5 rounded-xl border border-slate-200 bg-slate-50/60 p-3">
            <UiCheckbox id="gritsettings-settingsForm-use_report_history" bind:checked={settingsForm.use_report_history} label="Använd min skrivstil från Spiris" />
            <div class="flex-1">
              <div class="flex items-center gap-1.5 text-xs font-semibold text-slate-900">
                <span>Använd min skrivstil från Spiris</span>
                <HelpTip
                  label="Om skrivstil från Spiris"
                  text="Läser vald Spiris-användares senaste 90 dagars kommentarer före rapportdagen för språk, längd och detaljnivå. Används aldrig som bevis för nytt arbete."
                />
              </div>
              {#if settingsForm.use_report_history && !settingsForm.ai_consent}
                <p class="mt-1 text-[11px] font-medium text-amber-700">
                  Kräver även att AI-samtycke ovan är aktiverat (ditt val sparas).
                </p>
              {:else if settingsForm.use_report_history && settingsForm.user_id <= 0}
                <p class="mt-1 text-[11px] font-medium text-amber-700">
                  Välj en Spiris-användare under Spiris-fliken för att hämta skrivstil.
                </p>
              {/if}
            </div>
          </label>

          <div class="rounded-xl border border-slate-200 bg-slate-50/60 p-3">
            <button
              type="button"
              on:click={() => (showAdvancedAi = !showAdvancedAi)}
              aria-expanded={showAdvancedAi}
              class="flex w-full items-center justify-between text-left text-xs font-semibold text-slate-700"
            >
              <span>Avancerade AI-inställningar</span>
              <span class="inline-flex items-center gap-2 font-mono text-[11px] font-normal text-slate-500">
                <span>{settingsForm.model} · {settingsForm.timeout_seconds} s</span>
                <ChevronDown class="h-3.5 w-3.5 transition-transform {showAdvancedAi ? 'rotate-180' : ''}" aria-hidden="true" />
              </span>
            </button>

            {#if showAdvancedAi}
              <div transition:slide={{ duration: 180 }} class="mt-3 space-y-3 border-t border-slate-200 pt-3">
                <p class="rounded-lg bg-white p-2.5 text-xs text-slate-600">
                  {providerInfo.description}
                </p>
                <div class="grit-field-container">
                  <div class="grit-cols-2 grid gap-3">
                  <div>
                    <div class="mb-1 flex min-h-6 items-center">
                      <label for="grit-ai-model" class="text-xs font-medium text-slate-700">Modell</label>
                    </div>
                    <input
                      id="grit-ai-model"
                      type="text"
                      bind:value={settingsForm.model}
                      class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900"
                    />
                  </div>
                  <div>
                    <div class="mb-1 flex min-h-6 items-center">
                      <label for="grit-ai-timeout" class="text-xs font-medium text-slate-700">Tidsgräns (sekunder)</label>
                    </div>
                    <input
                      id="grit-ai-timeout"
                      type="number"
                      min="30"
                      step="30"
                      bind:value={settingsForm.timeout_seconds}
                      class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900"
                    />
                  </div>
                </div>
                </div>
              </div>
            {/if}
          </div>
        </div>

        <div class="flex justify-end pt-1">
          <button
            type="button"
            on:click={handleSaveSettings}
            disabled={savingSettings}
            class="inline-flex items-center gap-1.5 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
          >
            <Check class="h-3.5 w-3.5" aria-hidden="true" />
            <span>{savingSettings ? 'Sparar...' : 'Spara AI-inställningar'}</span>
          </button>
        </div>
      </section>
    {/if}

    {#if showSpiris}
      <section class="space-y-4 rounded-xl border border-slate-200 bg-white p-5">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="flex items-center gap-2">
            <Building2 class="h-4 w-4 text-slate-600" aria-hidden="true" />
            <div>
              <h2 class="text-base font-semibold text-slate-900">Spiris</h2>
              <p class="text-xs text-slate-500">Användare, debitering och skyddade inloggningsuppgifter.</p>
            </div>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <span class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium {connectionVerified
              ? 'bg-emerald-50 text-emerald-800'
              : status.credential_saved
                ? 'bg-slate-100 text-slate-700'
                : 'bg-amber-50 text-amber-800'}">
              {#if connectionVerified}
                <CheckCircle2 class="h-3.5 w-3.5 text-emerald-600" aria-hidden="true" />
                <span>Anslutning verifierad</span>
              {:else if status.credential_saved}
                <KeyRound class="h-3.5 w-3.5 text-slate-600" aria-hidden="true" />
                <span>Uppgifter sparade (ej testad)</span>
              {:else}
                <AlertCircle class="h-3.5 w-3.5 text-amber-600" aria-hidden="true" />
                <span>Ej konfigurerad</span>
              {/if}
            </span>

            <button
              type="button"
              on:click={handleTestConnection}
              disabled={testingConnection || !status.credential_saved}
              class="inline-flex items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-50"
            >
              <CheckCircle2 class="h-3.5 w-3.5" aria-hidden="true" />
              <span>{testingConnection ? 'Testar...' : 'Testa anslutning'}</span>
            </button>

            <button
              type="button"
              on:click={handleSyncSpiris}
              disabled={syncingSpiris || !status.credential_saved}
              class="inline-flex items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-50"
            >
              <RefreshCw class="h-3.5 w-3.5 {syncingSpiris ? 'animate-spin' : ''}" aria-hidden="true" />
              <span>{syncingSpiris ? 'Synkar...' : 'Synka Spiris'}</span>
            </button>
          </div>
        </div>

        {#if connectionResult}
          <div class="space-y-2 rounded-xl border border-slate-200 bg-slate-50/70 p-3.5 text-xs">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <span class="font-semibold {connectionResult.authentication.ok ? 'text-emerald-800' : 'text-rose-800'}">
                Autentisering: {connectionResult.authentication.ok ? 'OK' : 'Misslyckades'}
                {#if connectionResult.authentication.message}
                  · {connectionResult.authentication.message}
                {/if}
              </span>
              <span class="rounded-full bg-slate-200/70 px-2 py-0.5 text-[11px] font-medium text-slate-700">
                Skrivbehörighet: {connectionResult.submission_permission === 'not_tested' ? 'Ej testad (endast läsning)' : connectionResult.submission_permission}
              </span>
            </div>
            {#if connectionResult.reads.length > 0}
              <ul class="space-y-1 border-t border-slate-200/80 pt-2">
                {#each connectionResult.reads as check}
                  <li class="flex items-center justify-between gap-2">
                    <span class="font-medium text-slate-700">{resourceLabels[check.resource] ?? "Resurs"}</span>
                    <span class="font-mono {check.ok ? 'text-emerald-700' : 'text-rose-700'}">
                      {check.ok ? 'OK' : 'Fel'}{typeof check.count === 'number' ? ` (${check.count})` : ''}{check.message ? ` · ${check.message}` : ''}
                    </span>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <div class="text-xs font-medium text-slate-700">
            <div class="mb-1">Spiris-användare (krävs)</div>
            <UiSelect
              label="Spiris-användare"
              value={String(settingsForm.user_id)}
              options={userOptions}
              on:change={(e) => {
                const nextUserId = Number(e.detail) || 0;
                settingsForm = { ...settingsForm, user_id: nextUserId };
                if (verifiedUserId !== null && nextUserId !== verifiedUserId) {
                  invalidateVerification();
                }
              }}
            />
          </div>

          <label for="gritsettings-settingsForm-billable" class="flex cursor-pointer items-start gap-2.5 rounded-xl border border-slate-200 bg-slate-50/60 p-3">
            <UiCheckbox id="gritsettings-settingsForm-billable" bind:checked={settingsForm.billable} label="Fakturerbar tid" />
            <div>
              <div class="flex items-center gap-1.5 text-xs font-semibold text-slate-900">
                <span>Debiterbar tid</span>
                <HelpTip
                  label="Om debiterbar tid"
                  text="Styr om rapporterad tid markeras som fakturerbar (invoiceableHours) i Spiris."
                />
              </div>
              <p class="mt-0.5 text-xs text-slate-500">Fakturerbar som standard.</p>
            </div>
          </label>
        </div>

        <div class="flex justify-end">
          <button
            type="button"
            on:click={handleSaveSettings}
            disabled={savingSettings}
            class="inline-flex items-center gap-1.5 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
          >
            <Check class="h-3.5 w-3.5" aria-hidden="true" />
            <span>{savingSettings ? 'Sparar...' : 'Spara Spiris-val'}</span>
          </button>
        </div>

        <div class="space-y-3 border-t border-slate-100 pt-4">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <h3 class="text-xs font-semibold text-slate-800">Inloggningsuppgifter (sparas i nyckellagret)</h3>
            <div class="inline-flex rounded-xl bg-slate-100 p-1">
              <button
                type="button"
                on:click={() => (authMode = 'oauth')}
                class="rounded-lg px-3 py-1 text-xs font-semibold transition {authMode === 'oauth'
                  ? 'bg-white text-slate-900'
                  : 'text-slate-600'}"
              >
                App-id + hemlighet
              </button>
              <button
                type="button"
                on:click={() => (authMode = 'static')}
                class="rounded-lg px-3 py-1 text-xs font-semibold transition {authMode === 'static'
                  ? 'bg-white text-slate-900'
                  : 'text-slate-600'}"
              >
                Statisk token
              </button>
            </div>
          </div>

          {#if authMode === 'oauth'}
            <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <div>
                <div class="mb-1 flex min-h-6 items-center">
                  <label for="grit-oauth-appid" class="text-xs font-medium text-slate-700">App-id</label>
                </div>
                <input
                  id="grit-oauth-appid"
                  type="password"
                  autocomplete="off"
                  bind:value={applicationId}
                  placeholder={status.credential_saved ? '•••••••• (sparad)' : 'Ange App-id'}
                  class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900"
                />
              </div>
              <div>
                <div class="mb-1 flex min-h-6 items-center">
                  <label for="grit-oauth-secret" class="text-xs font-medium text-slate-700">Apphemlighet</label>
                </div>
                <input
                  id="grit-oauth-secret"
                  type="password"
                  autocomplete="off"
                  bind:value={secret}
                  placeholder={status.credential_saved ? '•••••••• (sparad)' : 'Ange Apphemlighet'}
                  class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900"
                />
              </div>
            </div>
          {:else}
            <label class="block text-xs font-medium text-slate-700">
              Statisk Bearer-token
              <input
                type="password"
                autocomplete="off"
                bind:value={staticToken}
                placeholder={status.credential_saved ? '•••••••• (sparad)' : 'Ange statisk token'}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900"
              />
            </label>
          {/if}

          <div class="flex justify-end">
            <button
              type="button"
              on:click={handleSaveCredentials}
              disabled={
                savingCredentials ||
                (authMode === 'oauth' ? !applicationId.trim() || !secret.trim() : !staticToken.trim())
              }
              class="inline-flex items-center gap-1.5 rounded-xl bg-slate-900 px-4 py-2 text-xs font-semibold text-white transition hover:bg-slate-800 disabled:opacity-50"
            >
              <KeyRound class="h-3.5 w-3.5" aria-hidden="true" />
              <span>{savingCredentials ? 'Sparar...' : 'Spara uppgifter'}</span>
            </button>
          </div>
        </div>
      </section>
    {/if}

    {#if showCapture}
      <div class="space-y-5">
        <CaptureSettings
          settings={status.capture}
          monitors={status.monitors}
          on:saved={handleCaptureSaved}
        />

        <ImageInterpretationSettings
          bind:settings={settingsForm.image_interpretation}
          aiConsent={savedSettings.ai_consent}
          dirty={settingsDirty}
          saving={savingSettings}
          on:save={handleSaveSettings}
        />

        <section class="grit-field-container space-y-4 rounded-xl border border-slate-200 bg-white p-5">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold text-slate-900">Långtidsarkiv & lokal mapp</h2>
              <p class="text-xs text-slate-500">
                Valfritt. Utan arkivmapp behålls bilder och loggar lokalt. Lokal rensning sker först efter verifierad arkivering.
              </p>
            </div>
            <button
              type="button"
              on:click={handleOpenLocalDir}
              disabled={openingLocalDir}
              class="inline-flex items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-50"
            >
              <FolderOpen class="h-3.5 w-3.5" aria-hidden="true" />
              <span>{openingLocalDir ? 'Öppnar...' : 'Öppna lokal bildmapp'}</span>
            </button>
          </div>

          <div class="grit-cols-3 grid gap-3">
            <div class="grit-col-span-2">
              <div class="mb-1 flex min-h-6 items-center">
                <label for="grit-archive-dir" class="text-xs font-medium text-slate-700">
                  Arkivmapp (valfri)
                </label>
              </div>
              <div class="flex items-center gap-2">
                <input
                  id="grit-archive-dir"
                  type="text"
                  bind:value={settingsForm.archive_dir}
                  placeholder={status.local_dir || 'Lokal arbetsmapp'}
                  class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900"
                />
                <button
                  type="button"
                  on:click={handlePickArchiveDir}
                  class="inline-flex h-10 shrink-0 items-center gap-1.5 rounded-xl border border-slate-200 bg-slate-50 px-3 text-xs font-semibold text-slate-700 hover:bg-slate-100"
                >
                  <FolderOpen class="h-3.5 w-3.5" aria-hidden="true" />
                  <span>Välj mapp</span>
                </button>
              </div>
            </div>

            <div>
              <div class="mb-1 flex min-h-6 items-center gap-1.5">
                <label for="grit-archive-retention" class="text-xs font-medium text-slate-700">
                  Arkivlagring (dagar)
                </label>
                <HelpTip
                  label="Om arkivlagring"
                  text="Standard: 90 dagar. Ofärdiga rapporter och deras underlag sparas alltid tills de har skickats eller hoppats över."
                />
              </div>
              <input
                id="grit-archive-retention"
                type="number"
                min="1"
                max="3650"
                step="1"
                bind:value={settingsForm.archive_retention_days}
                class="h-10 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900"
              />
            </div>
          </div>

          <div class="flex justify-end">
            <button
              type="button"
              on:click={handleSaveSettings}
              disabled={savingSettings}
              class="inline-flex items-center gap-1.5 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
            >
              <Check class="h-3.5 w-3.5" aria-hidden="true" />
              <span>{savingSettings ? 'Sparar...' : 'Spara arkivinställningar'}</span>
            </button>
          </div>
        </section>
      </div>
    {/if}
  {/if}
</div>

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

    .grit-col-span-2 {
      grid-column: span 2 / span 2;
    }
  }

  .grit-settings :is(button, input, select):focus-visible {
    outline: 2px solid #4f46e5;
    outline-offset: 2px;
  }

  @media (prefers-reduced-motion: reduce) {
    .grit-settings * {
      transition-duration: 0.01ms !important;
      animation-duration: 0.01ms !important;
    }
  }
</style>
