<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import CaptureSettings from './CaptureSettings.svelte';
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

  type AuthMode = 'oauth' | 'static';

  export let initialStatus: GritStatus | null = null;

  const dispatch = createEventDispatcher<{
    statusUpdated: GritStatus;
    saved: void;
  }>();

  let status: GritStatus | null = initialStatus;
  let settingsForm: GritSettingsModel = initialStatus
    ? { ...initialStatus.settings }
    : createDefaultSettings();
  let savedSettings = { ...settingsForm };
  function adoptStatus(next: GritStatus, force = false): void {
    const clean = JSON.stringify(settingsForm) === JSON.stringify(savedSettings);
    status = next;
    if (clean || force) { settingsForm = { ...next.settings }; savedSettings = { ...next.settings }; }
  }
  let loading = !initialStatus;
  let savingSettings = false;
  let savingCredentials = false;
  let syncingSpiris = false;
  let errorMessage: string | null = null;
  let noticeMessage: string | null = null;

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
      noticeMessage = `Spiris synkroniserad (${nextStatus.projects.length} projekt, ${nextStatus.time_codes.length} tidskoder).`;
      emitSaved();
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      syncingSpiris = false;
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

  function handleProviderSelect(nextProvider: GritProvider): void {
    settingsForm = applyProviderChange(settingsForm, nextProvider);
  }

  async function handleSaveSettings(): Promise<void> {
    errorMessage = null;
    noticeMessage = null;
    savingSettings = true;
    try {
      const latest = normalizeGritStatus(await invoke('grit_status')).settings;
      const merged = { ...latest };
      for (const key of Object.keys(savedSettings) as (keyof GritSettingsModel)[]) {
        if (JSON.stringify(settingsForm[key]) !== JSON.stringify(savedSettings[key])) {
          Object.assign(merged, { [key]: settingsForm[key] });
        }
      }
      await invoke('grit_save_settings', { settings: merged });
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
      await refreshStatus();
      noticeMessage = 'Spiris-uppgifterna har sparats i datorns nyckellager.';
      emitSaved();
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      savingCredentials = false;
    }
  }

  onMount(() => {
    void refreshStatus();
  });
</script>

<div class="space-y-5">
  {#if errorMessage}
    <div class="rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-800 dark:border-rose-800/60 dark:bg-rose-950/40 dark:text-rose-200">
      <div class="font-semibold">Fel</div>
      <p class="mt-0.5 whitespace-pre-wrap">{errorMessage}</p>
    </div>
  {/if}

  {#if noticeMessage}
    <div class="rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800 dark:border-emerald-800/60 dark:bg-emerald-950/40 dark:text-emerald-200">
      {noticeMessage}
    </div>
  {/if}

  {#if loading && !status}
    <div class="rounded-xl border border-slate-200/80 bg-white/90 p-8 text-center text-sm text-slate-500 dark:border-[#30363d] dark:bg-[#161b22]/90 dark:text-[#8b949e]">
      Laddar Grit Work-inställningar...
    </div>
  {:else if status}
    <CaptureSettings
      settings={status.capture}
      monitors={status.monitors}
      on:saved={handleCaptureSaved}
    />

    <div class="grid grid-cols-1 gap-5 lg:grid-cols-2">
      <!-- AI & Spiris defaults -->
      <section class="space-y-4 rounded-xl border border-slate-200/80 bg-white/90 p-5 shadow-sm dark:border-[#30363d] dark:bg-[#161b22]/90">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 class="text-base font-semibold text-slate-900 dark:text-[#e6edf3]">
              Generering & Spiris-standarder
            </h2>
            <p class="mt-0.5 text-xs text-slate-500 dark:text-[#8b949e]">
              Välj vilken lokal AI-CLI som skapar tidrapportutkast och vilka standardvärden som skickas till Spiris.
            </p>
          </div>
          <button
            type="button"
            on:click={handleSyncSpiris}
            disabled={syncingSpiris}
            class="shrink-0 rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#c9d1d9] dark:hover:bg-[#30363d]"
          >
            {syncingSpiris ? 'Synkar...' : 'Synka Spiris'}
          </button>
        </div>

        <div class="space-y-3">
          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              AI-leverantör (lokal CLI)
              <select
                value={settingsForm.provider}
                on:change={(e) => {
                  const val = e.currentTarget.value;
                  if (val === 'codex' || val === 'grok' || val === 'agy') {
                    handleProviderSelect(val);
                  }
                }}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value="codex">Codex CLI (OpenAI)</option>
                <option value="grok">Grok CLI (xAI)</option>
                <option value="agy">Antigravity CLI (Google Gemini)</option>
              </select>
            </label>
            <p class="mt-1.5 rounded-lg bg-slate-50 p-2.5 text-xs text-slate-600 dark:bg-[#0d1117] dark:text-[#8b949e]">
              {providerInfo.description}
            </p>
          </div>

          <label class="flex cursor-pointer items-start gap-2.5 rounded-xl border p-3 transition {settingsForm.ai_consent
            ? 'border-emerald-200/80 bg-emerald-50/50 dark:border-emerald-800/50 dark:bg-emerald-950/20'
            : 'border-amber-200/80 bg-amber-50/50 dark:border-amber-800/50 dark:bg-amber-950/20'}">
            <input
              type="checkbox"
              bind:checked={settingsForm.ai_consent}
              class="mt-0.5 h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
            />
            <div>
              <div class="text-xs font-semibold text-slate-900 dark:text-[#e6edf3]">
                Informerat samtycke till AI-analys ({providerInfo.name})
              </div>
              <p class="mt-0.5 text-xs leading-relaxed text-slate-600 dark:text-[#8b949e]">
                Jag samtycker till att skärmbilder och aktivitetsloggar skickas till vald inloggad CLI-leverantör ({providerInfo.name} · {providerInfo.vendor}, utan separata API-nycklar). När samtycke är aktiverat kan Grit Work även förbereda rapportutkast i bakgrunden för väntande dagar.
              </p>
            </div>
          </label>

          <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Modell
              <input
                type="text"
                bind:value={settingsForm.model}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              />
            </label>

            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Timeout (sekunder)
              <input
                type="number"
                min="30"
                step="30"
                bind:value={settingsForm.timeout_seconds}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              />
            </label>
          </div>

          <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <div class="sm:col-span-2">
              <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
                Arkivmapp (valfri)
                <div class="mt-1 flex gap-2">
                  <input
                    type="text"
                    bind:value={settingsForm.archive_dir}
                    placeholder={status.local_dir || 'Lokal arbetsmapp'}
                    class="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
                  />
                  <button
                    type="button"
                    on:click={handlePickArchiveDir}
                    class="shrink-0 rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-100 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#c9d1d9]"
                  >
                    Välj mapp
                  </button>
                </div>
              </label>
            </div>

            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Arkivlagring (dagar)
              <input
                type="number"
                min="1"
                max="3650"
                step="1"
                bind:value={settingsForm.archive_retention_days}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              />
              <span class="mt-1 block font-normal text-slate-500">Standard: 90 dagar. Ofärdiga rapporter och deras underlag sparas tills de har skickats eller hoppats över.</span>
            </label>
          </div>

          <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Spiris-användare (krävs)
              <select
                value={settingsForm.user_id}
                on:change={(e) =>
                  (settingsForm = { ...settingsForm, user_id: Number(e.currentTarget.value) || 0 })}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value={0}>Välj användare...</option>
                {#each status.users as user (user.id)}
                  <option value={user.id}>{user.name}</option>
                {/each}
              </select>
            </label>

            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Tidskod (krävs)
              <select
                value={settingsForm.time_code_id}
                on:change={(e) =>
                  (settingsForm = {
                    ...settingsForm,
                    time_code_id: Number(e.currentTarget.value) || 0,
                  })}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value={0}>Välj tidskod...</option>
                {#each status.time_codes as tc (tc.id)}
                  <option value={tc.id}>{tc.name}</option>
                {/each}
              </select>
            </label>

            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Standardaktivitet
              <select
                value={settingsForm.activity_id}
                on:change={(e) =>
                  (settingsForm = {
                    ...settingsForm,
                    activity_id: Number(e.currentTarget.value) || 0,
                  })}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value={0}>Ingen standard</option>
                {#each status.activities as act (act.id)}
                  <option value={act.id}>{act.name}</option>
                {/each}
              </select>
            </label>

            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Standardartikel
              <select
                value={settingsForm.article_id}
                on:change={(e) =>
                  (settingsForm = {
                    ...settingsForm,
                    article_id: Number(e.currentTarget.value) || 0,
                  })}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value={0}>Ingen standard</option>
                {#each status.articles as art (art.id)}
                  <option value={art.id}>{art.name}</option>
                {/each}
              </select>
            </label>
          </div>

          <label class="flex cursor-pointer items-start gap-2.5 rounded-xl border border-slate-200/80 bg-slate-50/70 p-3 dark:border-[#30363d] dark:bg-[#0d1117]">
            <input
              type="checkbox"
              bind:checked={settingsForm.billable}
              class="mt-0.5 h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
            />
            <div>
              <div class="text-xs font-semibold text-slate-900 dark:text-[#e6edf3]">
                Debiterbar tid
              </div>
              <p class="mt-0.5 text-xs text-slate-500 dark:text-[#8b949e]">
                Markera om den rapporterade tiden ska faktureras kunden.
              </p>
            </div>
          </label>
        </div>

        <div class="flex justify-end pt-2">
          <button
            type="button"
            on:click={handleSaveSettings}
            disabled={savingSettings}
            class="rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
          >
            {savingSettings ? 'Sparar...' : 'Spara inställningar'}
          </button>
        </div>
      </section>

      <!-- Spiris Credentials -->
      <section class="space-y-4 rounded-xl border border-slate-200/80 bg-white/90 p-5 shadow-sm dark:border-[#30363d] dark:bg-[#161b22]/90">
        <div>
          <h2 class="text-base font-semibold text-slate-900 dark:text-[#e6edf3]">
            Anslutning till Spiris
          </h2>
          <p class="mt-0.5 text-xs text-slate-500 dark:text-[#8b949e]">
            Dina uppgifter sparas skyddat i datorns nyckellager.
          </p>
        </div>

        <div class="inline-flex rounded-xl border border-slate-200 bg-slate-100/80 p-1 dark:border-[#30363d] dark:bg-[#0d1117]">
          <button
            type="button"
            on:click={() => (authMode = 'oauth')}
            class="rounded-lg px-3 py-1.5 text-xs font-semibold transition {authMode === 'oauth'
              ? 'bg-white text-slate-900 shadow-sm dark:bg-[#21262d] dark:text-[#e6edf3]'
              : 'text-slate-600 dark:text-[#8b949e]'}"
          >
            Application ID + Secret
          </button>
          <button
            type="button"
            on:click={() => (authMode = 'static')}
            class="rounded-lg px-3 py-1.5 text-xs font-semibold transition {authMode === 'static'
              ? 'bg-white text-slate-900 shadow-sm dark:bg-[#21262d] dark:text-[#e6edf3]'
              : 'text-slate-600 dark:text-[#8b949e]'}"
          >
            Statisk token
          </button>
        </div>

        {#if authMode === 'oauth'}
          <div class="space-y-3">
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Application ID
              <input
                type="password"
                autocomplete="off"
                bind:value={applicationId}
                placeholder={status.credential_saved ? '•••••••• (sparad)' : 'Ange Application ID'}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              />
            </label>
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Client Secret
              <input
                type="password"
                autocomplete="off"
                bind:value={secret}
                placeholder={status.credential_saved ? '•••••••• (sparad)' : 'Ange Client Secret'}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              />
            </label>
          </div>
        {:else}
          <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
            Statisk Bearer-token
            <input
              type="password"
              autocomplete="off"
              bind:value={staticToken}
              placeholder={status.credential_saved ? '•••••••• (sparad)' : 'Ange statisk token'}
              class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
            />
          </label>
        {/if}

        <div class="flex items-center justify-between pt-2">
          <span class="text-xs text-slate-500 dark:text-[#8b949e]">
            {status.credential_saved ? 'Uppgifterna är sparade i datorns nyckellager.' : 'Inga uppgifter sparade ännu.'}
          </span>
          <button
            type="button"
            on:click={handleSaveCredentials}
            disabled={
              savingCredentials ||
              (authMode === 'oauth' ? !applicationId.trim() || !secret.trim() : !staticToken.trim())
            }
            class="rounded-xl bg-slate-900 px-4 py-2 text-xs font-semibold text-white transition hover:bg-slate-800 disabled:opacity-50 dark:bg-[#238636] dark:hover:bg-[#2ea043]"
          >
            {savingCredentials ? 'Sparar...' : 'Spara uppgifter'}
          </button>
        </div>
      </section>
    </div>
  {/if}
</div>
