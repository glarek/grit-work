<script lang="ts">
  import UiCheckbox from "$lib/components/ui/UiCheckbox.svelte";
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { AlertCircle, CheckCircle2, KeyRound, ShieldCheck, Sparkles } from 'lucide-svelte';
  import UiDialog from '$lib/components/ui/UiDialog.svelte';
  import UiSelect from '$lib/components/ui/UiSelect.svelte';
  import {
    applyProviderChange,
    createDefaultSettings,
    normalizeGritStatus,
    PROVIDER_LABELS,
    type GritProvider,
    type GritSettings as GritSettingsModel,
    type GritStatus,
  } from './gritTypes.ts';

  const providerOptions = [
    { value: 'codex', label: 'Codex CLI (OpenAI)' },
    { value: 'grok', label: 'Grok CLI (xAI)' },
    { value: 'agy', label: 'Antigravity CLI (Google Gemini)' },
  ];

  type AuthMode = 'oauth' | 'static';

  export let open = true;

  const dispatch = createEventDispatcher<{
    complete: void;
    defer: void;
  }>();

  let status: GritStatus | null = null;
  let settingsForm: GritSettingsModel = createDefaultSettings();
  let authMode: AuthMode = 'oauth';
  let applicationId = '';
  let secret = '';
  let staticToken = '';

  let loading = true;
  let saving = false;
  let errorMessage: string | null = null;

  $: providerInfo = PROVIDER_LABELS[settingsForm.provider];
  $: hasCredentials = Boolean(
    status?.credential_saved ||
      (authMode === 'oauth' ? applicationId.trim() && secret.trim() : staticToken.trim()),
  );
  $: canComplete = Boolean(hasCredentials && settingsForm.ai_consent);

  function formatError(err: unknown): string {
    if (typeof err === 'string' && err.trim()) return err;
    if (err instanceof Error && err.message.trim()) return err.message;
    return 'Ett oväntat fel uppstod.';
  }

  function handleProviderSelect(nextProvider: GritProvider): void {
    settingsForm = applyProviderChange(settingsForm, nextProvider);
  }

  async function loadInitialStatus(): Promise<void> {
    loading = true;
    errorMessage = null;
    try {
      const raw = await invoke<unknown>('grit_status');
      const next = normalizeGritStatus(raw);
      status = next;
      // Preserve existing user settings without auto-granting ai_consent.
      settingsForm = { ...next.settings };
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      loading = false;
    }
  }

  async function handleComplete(): Promise<void> {
    if (!canComplete || saving) return;
    errorMessage = null;
    saving = true;
    try {
      const hasNewCreds =
        authMode === 'oauth'
          ? Boolean(applicationId.trim() && secret.trim())
          : Boolean(staticToken.trim());

      if (hasNewCreds) {
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
      }

      await invoke('grit_save_settings', { settings: settingsForm });
      dispatch('complete');
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      saving = false;
    }
  }

  function handleDefer(): void {
    if (saving) return;
    dispatch('defer');
  }

  onMount(() => {
    void loadInitialStatus();
  });
</script>

<UiDialog
  bind:open
  title="Anslut Spiris och godkänn AI-analys"
  description="Fyll i Spiris-uppgifter och välj AI-samtycke, eller klicka Avvakta."
  dismissible={false}
>
  <div class="grit-onboarding space-y-5">
    <div class="inline-flex items-center gap-1.5 rounded-full bg-indigo-50 px-2.5 py-0.5 text-xs font-medium text-indigo-700">
      <Sparkles class="h-3.5 w-3.5" aria-hidden="true" />
      <span>Kom igång med GRIT Work</span>
    </div>

      {#if errorMessage}
        <div role="alert" class="flex items-start gap-2 rounded-xl border border-rose-200 bg-rose-50 px-3.5 py-2.5 text-xs text-rose-800">
          <AlertCircle class="mt-0.5 h-4 w-4 shrink-0 text-rose-600" aria-hidden="true" />
          <span>{errorMessage}</span>
        </div>
      {/if}

      {#if loading}
        <div class="py-6 text-center text-xs text-slate-500">Laddar status...</div>
      {:else}
        <div class="space-y-4">
          <!-- Spiris credentials -->
          <section class="space-y-3 rounded-xl border border-slate-200 bg-slate-50/50 p-4">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <div class="flex items-center gap-2 text-xs font-semibold text-slate-900">
                <KeyRound class="h-4 w-4 text-slate-600" aria-hidden="true" />
                <span>1. Spiris-inloggning</span>
              </div>
              {#if status?.credential_saved}
                <span class="inline-flex items-center gap-1 rounded-full bg-emerald-50 px-2 py-0.5 text-[11px] font-medium text-emerald-700">
                  <CheckCircle2 class="h-3 w-3" aria-hidden="true" />
                  <span>Uppgifter sparade</span>
                </span>
              {:else}
                <div class="inline-flex rounded-lg bg-slate-100 p-0.5">
                  <button
                    type="button"
                    on:click={() => (authMode = 'oauth')}
                    class="rounded-md px-2.5 py-1 text-[11px] font-semibold transition {authMode === 'oauth'
                      ? 'bg-white text-slate-900'
                      : 'text-slate-600'}"
                  >
                    App-id + hemlighet
                  </button>
                  <button
                    type="button"
                    on:click={() => (authMode = 'static')}
                    class="rounded-md px-2.5 py-1 text-[11px] font-semibold transition {authMode === 'static'
                      ? 'bg-white text-slate-900'
                      : 'text-slate-600'}"
                  >
                    Statisk token
                  </button>
                </div>
              {/if}
            </div>

            {#if !status?.credential_saved}
              {#if authMode === 'oauth'}
                <div class="grid grid-cols-1 gap-2.5 sm:grid-cols-2">
                  <label class="block text-xs font-medium text-slate-700">
                    App-id
                    <input
                      type="password"
                      autocomplete="off"
                      bind:value={applicationId}
                      placeholder="Ange App-id"
                      class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900"
                    />
                  </label>
                  <label class="block text-xs font-medium text-slate-700">
                    Apphemlighet
                    <input
                      type="password"
                      autocomplete="off"
                      bind:value={secret}
                      placeholder="Ange Apphemlighet"
                      class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900"
                    />
                  </label>
                </div>
              {:else}
                <label class="block text-xs font-medium text-slate-700">
                  Statisk Bearer-token
                  <input
                    type="password"
                    autocomplete="off"
                    bind:value={staticToken}
                    placeholder="Ange statisk token"
                    class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900"
                  />
                </label>
              {/if}
            {/if}
          </section>

          <!-- AI provider + explicit consent -->
          <section class="space-y-3 rounded-xl border border-slate-200 bg-slate-50/50 p-4">
            <div class="flex items-center gap-2 text-xs font-semibold text-slate-900">
              <ShieldCheck class="h-4 w-4 text-slate-600" aria-hidden="true" />
              <span>2. AI-tjänst & informerat samtycke</span>
            </div>

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

            <label for="onboarding-settingsForm-ai_consent" class="flex cursor-pointer items-start gap-2.5 rounded-xl border bg-white p-3 {settingsForm.ai_consent
              ? 'border-emerald-200'
              : 'border-amber-200'}">
              <UiCheckbox id="onboarding-settingsForm-ai_consent" bind:checked={settingsForm.ai_consent} label={`Jag samtycker till AI-analys (${providerInfo.name})`} />
              <div>
                <div class="text-xs font-semibold text-slate-900">
                  Jag samtycker till AI-analys ({providerInfo.name})
                </div>
                <p class="mt-0.5 text-xs leading-relaxed text-slate-600">
                  Skärmbilder och aktivitetsloggar skickas till din inloggade CLI-leverantör ({providerInfo.name} · {providerInfo.vendor}, utan API-nycklar). Inga tidrapporter skickas till Spiris utan ditt godkännande.
                </p>
              </div>
            </label>
          </section>
        </div>
      {/if}

      <div class="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 pt-4">
        <button
          type="button"
          on:click={handleDefer}
          disabled={saving}
          class="rounded-xl border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-50"
        >
          Avvakta
        </button>

        <button
          type="button"
          on:click={handleComplete}
          disabled={!canComplete || saving || loading}
          class="inline-flex items-center gap-1.5 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          <CheckCircle2 class="h-3.5 w-3.5" aria-hidden="true" />
          <span>{saving ? 'Sparar...' : 'Slutför & börja'}</span>
        </button>
      </div>
  </div>
</UiDialog>

<style>
  .grit-onboarding :is(button, input, select):focus-visible {
    outline: 2px solid #4f46e5;
    outline-offset: 2px;
  }
</style>
