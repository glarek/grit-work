<script lang="ts">
  import { onMount } from 'svelte';
  import { mergeSettingsEdits } from './mergeSettings.ts';
  import type { ComponentProps } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { cache } from '../../lib/stores/cache.ts';
  import { locale, t } from '$lib/i18n/index.ts';
  import { formatUserError } from '$lib/utils/errorDisplay.ts';
  import { showToast } from '../../lib/stores/toast.ts';

  import { AlertCircle, Check, CheckCircle2, KeyRound, Settings as SettingsIcon, Sparkles, Building2, Camera, SlidersHorizontal, Shield } from 'lucide-svelte';

  import GritSettings from '../grit/GritSettings.svelte';
  import SettingsGeneral from './components/SettingsGeneral.svelte';
  import SettingsSystem from './components/SettingsSystem.svelte';
  import SettingsPrivacy from './components/SettingsPrivacy.svelte';

  type GeneralConfig = ComponentProps<SettingsGeneral>['config'];
  type PrivacyConfig = ComponentProps<SettingsPrivacy>['config'];

  interface LegacyModelConfig {
    provider: string;
    endpoint: string;
    api_key: string | null;
    model: string;
    vision_model?: string;
  }

  interface RootSettingsFields {
    ai_provider: LegacyModelConfig;
    text_model?: unknown;
    text_model_profiles: unknown[];
    text_model_provider_cache?: Record<string, unknown>;
    daily_report_custom_prompt: string;
    daily_report_prompt_presets: unknown[];
    daily_report_export_dir?: string | null;
    daily_report_auto_export?: boolean;
    vision_model: LegacyModelConfig;
    app_category_rules: unknown[];
    privacy: PrivacyConfig['privacy'] & { sensitive_keywords?: string[] };
    storage: {
      screenshot_retention_days: number;
      metadata_cleanup_enabled: boolean;
      metadata_retention_days: number;
      storage_limit_mb: number;
      jpeg_quality: number;
      max_image_width: number;
      screenshots_enabled: boolean;
      screenshot_display_mode: string;
      screenshot_width_mode: string;
    };
  }

  type SettingsConfig = GeneralConfig
    & PrivacyConfig
    & RootSettingsFields
    & { ui_visual_style: string };

  type DraftPrivacySettings = Partial<PrivacyConfig['privacy']> & {
    sensitive_keywords?: string[];
  };

  type DraftSettingsConfig = Omit<
    SettingsConfig,
    | 'ai_provider'
    | 'text_model'
    | 'text_model_profiles'
    | 'daily_report_custom_prompt'
    | 'daily_report_prompt_presets'
    | 'vision_model'
    | 'storage'
    | 'app_category_rules'
    | 'privacy'
    | 'ui_visual_style'
  > & {
    ai_provider: LegacyModelConfig | null;
    text_model?: unknown;
    text_model_profiles?: unknown[];
    daily_report_custom_prompt?: string;
    daily_report_prompt_presets?: unknown[];
    vision_model: LegacyModelConfig | null;
    storage: Partial<RootSettingsFields['storage']> | null;
    app_category_rules?: unknown[];
    privacy: DraftPrivacySettings | null;
    ui_visual_style: string;
  };

  type SettingsTabId = 'ai' | 'spiris' | 'capture' | 'general' | 'privacy';

  interface SettingsTab {
    id: SettingsTabId;
    label: string;
    icon: SettingsTabId;
  }

  let config: SettingsConfig | null = null;
  let baselineConfig: SettingsConfig | null = null;
  let loading = true;
  let saving = false;
  let dirty = false;
  let error: string | null = null;
  let success = false;
  let runningApps: string[] = [];
  let recentApps: string[] = [];
  let settingsRuntimePlatform = '';
  let successTimer: ReturnType<typeof setTimeout> | null = null;
  let aiReady = false;
  let spirisConfigured = false;
  let spirisVerified = false;
  $: currentLocale = $locale;

  let activeTab: SettingsTabId = 'general';

  const tabs: SettingsTab[] = [
    { id: 'general', label: 'Allmänt & system', icon: 'general' },
    { id: 'ai', label: 'AI & rapportering', icon: 'ai' },
    { id: 'spiris', label: 'Spiris', icon: 'spiris' },
    { id: 'capture', label: 'Insamling & lagring', icon: 'capture' },
    { id: 'privacy', label: 'Integritet', icon: 'privacy' },
  ];

  // 加载配置
  async function loadConfig() {
    loading = true;
    error = null;
    try {
      const [loadedConfig, loadedRuntimePlatform] = await Promise.all([
        invoke<DraftSettingsConfig>('get_config'),
        invoke<string>('get_runtime_platform'),
      ]);

      settingsRuntimePlatform = loadedRuntimePlatform;

      // 确保对象存在
      if (!loadedConfig.ai_provider) {
        loadedConfig.ai_provider = { provider: 'ollama', endpoint: 'http://localhost:11434', api_key: null, model: 'llava', vision_model: 'llava' };
      }
      if (!loadedConfig.text_model) {
        loadedConfig.text_model = { provider: 'ollama', endpoint: 'http://localhost:11434', api_key: null, model: 'qwen2.5' };
      }
      if (!loadedConfig.text_model_profiles) {
        loadedConfig.text_model_profiles = [];
      }
      if (!loadedConfig.text_model_provider_cache) {
        loadedConfig.text_model_provider_cache = {};
      }
      if (typeof loadedConfig.daily_report_custom_prompt !== 'string') {
        loadedConfig.daily_report_custom_prompt = '';
      }
      if (typeof loadedConfig.daily_report_export_dir !== 'string' && loadedConfig.daily_report_export_dir !== null) {
        loadedConfig.daily_report_export_dir = null;
      }
      if (typeof loadedConfig.daily_report_auto_export !== 'boolean') {
        loadedConfig.daily_report_auto_export = false;
      }
      if (!Array.isArray(loadedConfig.daily_report_prompt_presets)) {
        loadedConfig.daily_report_prompt_presets = [];
      }
      if (!['a', 'b', 'c'].includes(loadedConfig.ui_visual_style)) {
        loadedConfig.ui_visual_style = 'c';
      }
      if (!loadedConfig.vision_model) {
        loadedConfig.vision_model = { provider: 'ollama', endpoint: 'http://localhost:11434', api_key: null, model: 'llava' };
      }
      if (typeof loadedConfig.lightweight_mode !== 'boolean') {
        loadedConfig.lightweight_mode = false;
      }
      if (typeof loadedConfig.auto_start_silent !== 'boolean') {
        loadedConfig.auto_start_silent = false;
      }
      if (!loadedConfig.storage) {
        loadedConfig.storage = {
          screenshot_retention_days: 7,
          metadata_cleanup_enabled: false,
          metadata_retention_days: 30,
          storage_limit_mb: 2048,
          jpeg_quality: 85,
          max_image_width: 1280,
          screenshots_enabled: true,
          screenshot_display_mode: 'active_window',
          screenshot_width_mode: 'auto',
        };
      }
      if (typeof loadedConfig.storage.metadata_cleanup_enabled !== 'boolean') {
        loadedConfig.storage.metadata_cleanup_enabled = false;
      }
      if (typeof loadedConfig.storage.screenshots_enabled !== 'boolean') {
        loadedConfig.storage.screenshots_enabled = true;
      }
      if (!loadedConfig.storage.screenshot_display_mode) {
        loadedConfig.storage.screenshot_display_mode = 'active_window';
      }
      if (!loadedConfig.storage.screenshot_width_mode || !['auto', 'fixed'].includes(loadedConfig.storage.screenshot_width_mode)) {
        loadedConfig.storage.screenshot_width_mode = 'auto';
      }
      if (!loadedConfig.app_category_rules) loadedConfig.app_category_rules = [];
      if (!loadedConfig.privacy) loadedConfig.privacy = {};
      if (!loadedConfig.privacy.app_rules) loadedConfig.privacy.app_rules = [];
      if (!loadedConfig.privacy.excluded_keywords) loadedConfig.privacy.excluded_keywords = [];
      delete loadedConfig.privacy.sensitive_keywords;

      config = loadedConfig as SettingsConfig;
      baselineConfig = JSON.parse(JSON.stringify(config)) as SettingsConfig;
      cache.setConfig(config);
    } catch (e) {
      error = formatUserError(e, t('common.loadFailedRetry'));
      console.error('加载配置失败:', e);
      settingsRuntimePlatform = '';
    } finally {
      loading = false;
    }
  }

  // 加载运行中的应用
  async function loadRunningApps() {
    try {
      runningApps = await invoke<string[]>('get_running_apps');
    } catch (e) {
      console.error('获取运行应用失败:', e);
      runningApps = [];
    }
  }

  // 加载历史应用列表
  async function loadRecentApps() {
    try {
      recentApps = await invoke<string[]>('get_recent_apps');
    } catch (e) {
      console.error('获取历史应用失败:', e);
      recentApps = [];
    }
  }

  // 保存配置：先读取最新后端配置，仅合并用户实际修改过的字段（含跨标签页未保存修改），避免覆盖 Grit 采集/存储或其他并发更新
  async function saveConfig() {
    if (!config) return;
    const currentConfig = config;
    const baseline = baselineConfig ?? currentConfig;
    saving = true;
    error = null;
    success = false;

    try {
      const latestConfig = await invoke<SettingsConfig>('get_config');
      const mergedConfig = mergeSettingsEdits(latestConfig, currentConfig, baseline);

      await invoke<void>('save_config', { config: mergedConfig });
      config = mergedConfig;
      baselineConfig = JSON.parse(JSON.stringify(mergedConfig)) as SettingsConfig;
      success = true;
      dirty = false;
      cache.setConfig(mergedConfig);
      showToast(t('settings.saveSuccessToast'), 'success');
      
      if (successTimer !== null) clearTimeout(successTimer);
      successTimer = setTimeout(() => {
        success = false;
        successTimer = null;
      }, 3000);
    } catch (e) {
      error = formatUserError(e, t('common.loadFailedRetry'));
    } finally {
      saving = false;
    }
  }

  function handleSettingsChange(event: CustomEvent<unknown>) {
    const detail = event.detail;
    const autosaved = typeof detail === 'object'
      && detail !== null
      && 'autosaved' in detail
      && detail.autosaved === true;
    if (autosaved && config) {
      baselineConfig = JSON.parse(JSON.stringify(config)) as SettingsConfig;
    }
    dirty = !autosaved;
  }

  function handleGritConnectionState(
    event: CustomEvent<{ aiReady: boolean; credentialSaved: boolean; connectionVerified: boolean }>,
  ) {
    aiReady = event.detail.aiReady;
    spirisConfigured = event.detail.credentialSaved;
    spirisVerified = event.detail.connectionVerified;
  }

  async function handleGritSaved() {
    if (dirty) return;
    try {
      const latestConfig = await invoke<SettingsConfig>('get_config');
      config = latestConfig;
      baselineConfig = JSON.parse(JSON.stringify(latestConfig)) as SettingsConfig;
      cache.setConfig(latestConfig);
    } catch (e) {
      console.warn('Kunde inte uppdatera konfiguration efter Grit-sparning:', e);
    }
  }

  onMount(() => {
    const unsubscribeCache = cache.subscribe((state) => {
      if (!state.config) return;
      // 保存中或用户已编辑配置时，不覆盖（避免丢弃未保存的修改）
      if (saving) return;
      if (config && dirty) return;
      config = state.config as SettingsConfig;
    });

    loadConfig();
    loadRunningApps();
    loadRecentApps();

    return () => {
      unsubscribeCache();
      if (successTimer !== null) clearTimeout(successTimer);
    };
  });
</script>

<div class="page-shell settings-editorial-shell" data-locale={currentLocale}>
  <div class="page-header page-axis-operation">
    <div class="page-title-group">
      <div class="page-title-badge">
        <SettingsIcon class="h-4 w-4" aria-hidden="true" />
      </div>
      <div class="page-title-copy">
        <h2>Inställningar</h2>
        <p>Hantera AI, Spiris, skärmbilder, system och integritet i GRIT Work.</p>
      </div>
    </div>

    <!-- Spara -->
    <div class="settings-save-dock">
      {#if activeTab === 'general' || activeTab === 'privacy'}
      <button
        on:click={saveConfig}
        disabled={loading || saving}
        class="inline-flex items-center gap-1.5 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 disabled:opacity-50"
      >
        {#if saving}
          <div class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-white border-t-transparent"></div>
          <span>Sparar...</span>
        {:else if success}
          <Check class="h-3.5 w-3.5" aria-hidden="true" />
          <span>Sparad</span>
        {:else}
          <span>Spara</span>
        {/if}
      </button>
      {/if}
    </div>
  </div>

  {#if loading}
    <div class="flex justify-center py-12">
      <div class="h-8 w-8 animate-spin rounded-full border-b-2 border-indigo-600"></div>
    </div>
  {:else if error}
    <div class="page-banner-error mb-6">
      <div>
        <p class="font-semibold">{t('settings.loadError')}</p>
        <p class="mt-1 text-sm">{error}</p>
      </div>
      <button on:click={loadConfig} class="page-action-brand">{t('settings.retry')}</button>
    </div>
  {:else if config}
    <div class="w-full settings-editorial-board page-axis-operation">
      {#if settingsRuntimePlatform === 'macos'}
        <div class="settings-card settings-top-status-zone">
          <SettingsSystem />
        </div>
      {/if}

      <div class="settings-stage-layout grid grid-cols-1 gap-6 md:grid-cols-[220px_minmax(0,1fr)]">
        <nav class="settings-tab-rail flex flex-row gap-1 overflow-x-auto border-0 bg-transparent p-0 shadow-none md:flex-col" aria-label={t('settings.title')}>
          {#each tabs as tab}
            <button
              type="button"
              on:click={() => activeTab = tab.id}
              class="settings-tab-rail-item flex shrink-0 items-center gap-2.5 rounded-xl border-0 px-3 py-2 text-left text-xs font-medium shadow-none transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 {activeTab === tab.id
                ? 'settings-tab-rail-item-active bg-slate-100 font-semibold text-slate-900'
                : 'bg-transparent text-slate-600 hover:bg-slate-50 hover:text-slate-900'}"
              aria-current={activeTab === tab.id ? 'page' : undefined}
            >
              <span class="settings-tab-rail-icon text-slate-500">
                {#if tab.icon === 'ai'}
                  <Sparkles class="h-4 w-4" aria-hidden="true" />
                {:else if tab.icon === 'spiris'}
                  <Building2 class="h-4 w-4" aria-hidden="true" />
                {:else if tab.icon === 'capture'}
                  <Camera class="h-4 w-4" aria-hidden="true" />
                {:else if tab.icon === 'general'}
                  <SlidersHorizontal class="h-4 w-4" aria-hidden="true" />
                {:else if tab.icon === 'privacy'}
                  <Shield class="h-4 w-4" aria-hidden="true" />
                {/if}
              </span>
              <span class="flex-1 whitespace-nowrap">{tab.label}</span>
              {#if tab.id === 'ai'}
                {#if aiReady}
                  <CheckCircle2 class="ml-auto h-3.5 w-3.5 shrink-0 text-emerald-600" aria-label="AI-samtycke aktivt" />
                {:else}
                  <AlertCircle class="ml-auto h-3.5 w-3.5 shrink-0 text-amber-600" aria-label="AI-samtycke saknas" />
                {/if}
              {:else if tab.id === 'spiris'}
                {#if spirisVerified}
                  <CheckCircle2 class="ml-auto h-3.5 w-3.5 shrink-0 text-emerald-600" aria-label="Spiris läskontroll verifierad" />
                {:else if spirisConfigured}
                  <KeyRound class="ml-auto h-3.5 w-3.5 shrink-0 text-slate-500" aria-label="Uppgifter sparade (ej testad)" />
                {:else}
                  <AlertCircle class="ml-auto h-3.5 w-3.5 shrink-0 text-amber-600" aria-label="Spiris ej konfigurerad" />
                {/if}
              {/if}
            </button>
          {/each}
        </nav>

        <div class="settings-stage-shell min-w-0">
          <div class:hidden={activeTab !== 'ai' && activeTab !== 'spiris' && activeTab !== 'capture'}>
            <GritSettings
              section={activeTab === 'spiris' || activeTab === 'capture' ? activeTab : 'ai'}
              on:saved={handleGritSaved}
              on:connectionState={handleGritConnectionState}
            />
          </div>
          {#if activeTab === 'general'}
            <SettingsGeneral bind:config runtimePlatform={settingsRuntimePlatform} showAutoReport={false} on:change={() => dirty = true} />
          {:else if activeTab === 'privacy'}
            <SettingsPrivacy
              bind:config
              {runningApps}
              {recentApps}
              on:change={handleSettingsChange}
              on:refresh-apps={() => { loadRunningApps(); loadRecentApps(); }}
            />
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>
