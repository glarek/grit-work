<script lang="ts">
  import UiCheckbox from "$lib/components/ui/UiCheckbox.svelte";
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Check } from 'lucide-svelte';
  import { formatDurationLocalized, locale, t } from '$lib/i18n/index.ts';
  import UiSwitch from '../../../lib/components/ui/UiSwitch.svelte';
  import CollapsibleSection from '../../../lib/components/CollapsibleSection.svelte';

  interface WorkTimeSegment {
    start_hour: number;
    start_minute: number;
    end_hour: number;
    end_minute: number;
  }

  interface GeneralConfig {
    auto_start: boolean;
    auto_start_silent: boolean;
    hide_dock_icon: boolean;
    lightweight_mode: boolean;
    work_time_enabled: boolean;
    work_time_segments?: WorkTimeSegment[] | null;
    work_start_hour: number;
    work_start_minute: number;
    work_end_hour: number;
    work_end_minute: number;
    standard_work_hours?: number | null;
    idle_threshold_minutes: number;
    daily_work_goal_minutes: number | null;
    goal_notifications: boolean;
    memory_enabled: boolean;
    daily_report_auto_generate_time: string | null;
  }

  export let config: GeneralConfig;
  export let showAutoReport = true;
  export let runtimePlatform = "";

  const dispatch = createEventDispatcher<{ change: GeneralConfig }>();
  $: currentLocale = $locale;
  let autoStartEnabled = false;
  let autoStartBusy = false;

  onMount(async () => {
    try {
      autoStartEnabled = await invoke<boolean>('is_autostart_enabled');
      if (config.auto_start !== autoStartEnabled) {
        config.auto_start = autoStartEnabled;
        try {
          await invoke<void>('save_config', { config });
        } catch (e) {
          console.error('对齐注册表自启状态时写盘失败:', e);
        }
        dispatch('change', config);
      }
    } catch (e) {
      console.error('查询自启动状态失败:', e);
    }
  });

  function handleChange() {
    dispatch('change', config);
  }

  async function toggleAutoStart(targetState = !autoStartEnabled) {
    autoStartBusy = true;
    try {
      if (targetState) {
        await invoke<void>('enable_autostart', { silent: !!config.auto_start_silent });
      } else {
        await invoke<void>('disable_autostart');
      }
    } catch (e) {
      console.warn(`切换系统自启失败/警告 (目标状态: ${targetState}):`, e);
    }
    try {
      autoStartEnabled = await invoke<boolean>('is_autostart_enabled');
      config.auto_start = autoStartEnabled;
      try {
        await invoke<void>('save_config', { config });
      } catch (e) {
        console.error('保存开机自启状态失败:', e);
      }
      dispatch('change', config);
    } catch (e) {
      console.error('Autostartstatus kunde inte kontrolleras:', e);
      autoStartEnabled = !targetState;
    } finally { autoStartBusy = false; }
  }

  async function toggleDockIcon() {
    config.hide_dock_icon = !config.hide_dock_icon;
    try {
      await invoke<void>('set_dock_visibility', { visible: !config.hide_dock_icon });
    } catch (e) {
      console.error('设置 Dock 图标失败:', e);
    }
    dispatch('change', config);
  }

  function toggleLightweightMode() {
    config.lightweight_mode = !config.lightweight_mode;
    dispatch('change', config);
  }

  async function updateAutoStartLaunchMode(silentMode: boolean) {
    config.auto_start_silent = silentMode;
    try {
      await invoke<void>('save_config', { config });
    } catch (e) {
      console.error('保存启动模式失败:', e);
    }
    if (autoStartEnabled) {
      try {
        await invoke<void>('enable_autostart', { silent: silentMode });
      } catch (e) {
        console.error('更新自启动参数失败:', e);
      }
    }
    dispatch('change', config);
  }
</script>

<div class="settings-card" data-locale={currentLocale}>
  <h3 class="settings-card-title">{t('settingsGeneral.title')}</h3>

  <div class="settings-section">
    <!-- Aktivitetsgränser (kalendardygn) -->
    <div class="settings-block">
      <p class="settings-muted">All aktiv datortid loggas per kalenderdygn utan arbetspassgränser.</p>

      <!-- 高级工时设置（折叠） -->
      <CollapsibleSection title={t('settingsGeneral.advancedWorkSettings')} storageKey="settings.general.advancedWork">
        <!-- 标准工时（加班计算基准） -->
        <div class="flex items-center justify-between mt-3">
          <div>
            <span class="settings-text text-sm">{t('settingsGeneral.standardWorkHours')}</span>
            <p class="settings-muted mt-0.5">{t('settingsGeneral.standardWorkHoursHint')}</p>
          </div>
          <div class="flex items-center gap-2">
            <input
              type="number"
              aria-label={t('settingsGeneral.standardWorkHours')}
              min="1"
              max="24"
              step="0.5"
              value={config.standard_work_hours ?? 8}
              on:change={(e) => {
                // 非法值不再静默忽略(此前输入 30 会保留旧值但输入框仍显示 30):
                // 数字越界就 clamp 到 1~24,非数字回退当前值,并把结果回写到输入框
                const val = parseFloat(e.currentTarget.value);
                if (isNaN(val)) {
                  e.currentTarget.value = String(config.standard_work_hours ?? 8);
                  return;
                }
                const clamped = Math.min(24, Math.max(1, val));
                e.currentTarget.value = String(clamped);
                if (config.standard_work_hours !== clamped) {
                  config.standard_work_hours = clamped;
                  handleChange();
                }
              }}
              class="w-20 rounded-md border border-slate-200 bg-white px-2.5 py-1.5 text-center text-sm font-mono text-slate-900 focus:border-primary-400 focus:outline-none dark:border-[#30363d] dark:bg-[#161b22] dark:text-[#e6edf3]"
            />
            <span class="text-xs text-slate-500 dark:text-[#7d8590]">{t('settingsGeneral.hours')}</span>
          </div>
        </div>

        <!-- 空闲检测阈值 -->
        <div class="flex items-center justify-between mt-3">
          <div>
            <span class="settings-text text-sm">{t('settingsGeneral.idleThreshold')}</span>
            <p class="settings-muted mt-0.5">{t('settingsGeneral.idleThresholdHint')}</p>
          </div>
          <div class="flex items-center gap-2">
            <input
              type="number"
              aria-label={t('settingsGeneral.idleThreshold')}
              min="1"
              max="60"
              step="1"
              bind:value={config.idle_threshold_minutes}
              on:change={() => {
                config.idle_threshold_minutes = Math.max(1, Math.min(60, Number(config.idle_threshold_minutes) || 5));
                handleChange();
              }}
              class="w-16 rounded-md border border-slate-200 bg-white px-2 py-1 text-center text-sm dark:border-[#484f58] dark:bg-[#21262d]"
            />
            <span class="text-xs settings-subtle">{t('settingsGeneral.minutesUnit')}</span>
          </div>
        </div>
      </CollapsibleSection>
    </div>

    <!-- 工作目标 -->
    <CollapsibleSection title={t('settingsGeneral.workGoalTitle')} storageKey="settings.general.workGoal">
      <div class="flex items-center justify-between mt-3">
        <div>
          <span class="settings-text text-sm">{t('settingsGeneral.workGoalHours')}</span>
          <p class="settings-muted mt-0.5">{t('settingsGeneral.workGoalHint')}</p>
        </div>
        <div class="flex items-center gap-2">
          <input
            type="number"
            aria-label={t('settingsGeneral.workGoalHours')}
            min="0"
            max="16"
            step="0.5"
            value={config.daily_work_goal_minutes ? config.daily_work_goal_minutes / 60 : 0}
            on:change={(e) => {
              const hours = parseFloat(e.currentTarget.value);
              config.daily_work_goal_minutes = (!isNaN(hours) && hours > 0) ? Math.round(hours * 60) : null;
              handleChange();
            }}
            class="w-20 rounded-md border border-slate-200 bg-white px-2.5 py-1.5 text-center text-sm font-mono text-slate-900 focus:border-primary-400 focus:outline-none dark:border-[#30363d] dark:bg-[#161b22] dark:text-[#e6edf3]"
          />
          <span class="text-xs text-slate-500 dark:text-[#7d8590]">{t('settingsGeneral.hours')}</span>
        </div>
      </div>
      <label for="settingsgeneral-config-goal_notifications" class="flex items-center justify-between mt-3 cursor-pointer">
        <span class="settings-text text-sm">{t('settingsGeneral.workGoalNotifications')}</span>
        <UiCheckbox id="settingsgeneral-config-goal_notifications" bind:checked={config.goal_notifications} label={t('settingsGeneral.workGoalNotifications')} on:change={handleChange} />
      </label>
    </CollapsibleSection>

    <!-- AI 工作记忆 -->
    <CollapsibleSection title={t('settingsGeneral.memoryTitle')} storageKey="settings.general.memory">
      <label for="settingsgeneral-config-memory_enabled" class="flex items-center justify-between mt-3 cursor-pointer">
        <div>
          <span class="settings-text text-sm">{t('settingsGeneral.memoryEnabled')}</span>
          <p class="settings-muted mt-0.5">{t('settingsGeneral.memoryHint')}</p>
        </div>
        <UiCheckbox id="settingsgeneral-config-memory_enabled" bind:checked={config.memory_enabled} label={t('settingsGeneral.memoryEnabled')} on:change={handleChange} />
      </label>
    </CollapsibleSection>
    {#if showAutoReport}
    <div class="settings-block pt-4 border-t border-slate-200 dark:border-[#30363d]">
      <div class="flex flex-wrap items-center gap-3">
        <span class="settings-text">{t('settingsGeneral.reportAutoGenerateTime')}</span>
        <div class="control-inline">
          <input
            type="time"
            aria-label={t('settingsGeneral.reportAutoGenerateTime')}
            value={config.daily_report_auto_generate_time ?? ''}
            on:change={(e) => {
              config.daily_report_auto_generate_time = e.currentTarget.value || null;
              dispatch('change', config);
            }}
            class="w-20 bg-transparent text-sm font-mono text-slate-900 dark:text-[#e6edf3] focus:outline-none"
          />
        </div>
        {#if config.daily_report_auto_generate_time}
          <button
            type="button"
            class="inline-flex items-center gap-1 px-2.5 py-1.5 text-xs rounded-lg text-rose-500 hover:bg-rose-50 dark:text-rose-400 dark:hover:bg-rose-950/30 transition-colors"
            on:click={() => {
              config.daily_report_auto_generate_time = null;
              dispatch('change', config);
            }}
          >
            {t('settingsGeneral.reportAutoGenerateReset')}
          </button>
        {/if}
      </div>
      <p class="settings-note">{t('settingsGeneral.reportAutoGenerateTimeHint')}</p>
    </div>
    {/if}

    <!-- 系统行为 -->
    <div class="settings-block pt-4 border-t border-slate-200 dark:border-[#30363d]">
      <div class="space-y-2.5">
        <div class="settings-row">
          <span class="settings-text">{t('settingsGeneral.autoStart')}</span>
          <UiSwitch bind:checked={autoStartEnabled} label={t('settingsGeneral.autoStart')} disabled={autoStartBusy} on:change={(e)=>toggleAutoStart(e.detail)} />
        </div>

        {#if autoStartEnabled}
          <div class="rounded-xl border border-slate-200 bg-slate-50/70 p-3 dark:border-[#30363d] dark:bg-[#161b22]/40">
            <span class="settings-label">{t('settingsGeneral.autoStartLaunchMode')}</span>
            <div class="mt-2 flex flex-wrap gap-2">
              <button
                type="button"
                aria-pressed={!config.auto_start_silent}
                on:click={() => updateAutoStartLaunchMode(false)}
                class="inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs transition {!config.auto_start_silent
                  ? 'border-indigo-600 bg-indigo-50 font-semibold text-indigo-900 dark:border-indigo-500 dark:bg-indigo-950/40 dark:text-indigo-200'
                  : 'border-slate-200 bg-white text-slate-600 hover:bg-slate-50 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#8b949e]'}"
              >
                <span class="inline-flex h-3.5 w-3.5 items-center justify-center rounded-full border {!config.auto_start_silent ? 'border-indigo-600 bg-indigo-600 text-white' : 'border-slate-300 dark:border-[#484f58]'}" aria-hidden="true">
                  {#if !config.auto_start_silent}<Check size={10} strokeWidth={2.5} />{/if}
                </span>
                <span>{t('settingsGeneral.autoStartLaunchShow')}</span>
              </button>
              <button
                type="button"
                aria-pressed={!!config.auto_start_silent}
                on:click={() => updateAutoStartLaunchMode(true)}
                class="inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs transition {config.auto_start_silent
                  ? 'border-indigo-600 bg-indigo-50 font-semibold text-indigo-900 dark:border-indigo-500 dark:bg-indigo-950/40 dark:text-indigo-200'
                  : 'border-slate-200 bg-white text-slate-600 hover:bg-slate-50 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#8b949e]'}"
              >
                <span class="inline-flex h-3.5 w-3.5 items-center justify-center rounded-full border {config.auto_start_silent ? 'border-indigo-600 bg-indigo-600 text-white' : 'border-slate-300 dark:border-[#484f58]'}" aria-hidden="true">
                  {#if config.auto_start_silent}<Check size={10} strokeWidth={2.5} />{/if}
                </span>
                <span>{t('settingsGeneral.autoStartLaunchSilent')}</span>
              </button>
            </div>
          </div>
        {/if}

        {#if runtimePlatform === "macos"}
        <div class="settings-row">
          <span class="settings-text">{t('settingsGeneral.hideDockIcon')}</span>
          <UiSwitch bind:checked={config.hide_dock_icon} label={t('settingsGeneral.hideDockIcon')}  on:change={async()=>{ await invoke("set_dock_visibility",{visible:!config.hide_dock_icon}); handleChange(); }} />
        </div>
        {/if}

        <div class="settings-row">
          <div>
            <span class="settings-text">{t('settingsGeneral.lightweightMode')}</span>
            <p class="settings-muted mt-0.5">{t('settingsGeneral.lightweightModeDescription')}</p>
          </div>
          <UiSwitch bind:checked={config.lightweight_mode} label={t('settingsGeneral.lightweightMode')}  on:change={handleChange} />
        </div>
      </div>
    </div>
  </div>
</div>
