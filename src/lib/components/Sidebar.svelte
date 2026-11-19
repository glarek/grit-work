<script lang="ts">
  import { location, link } from 'svelte-spa-router';
  import { invoke } from '@tauri-apps/api/core';
  import { onDestroy, onMount } from 'svelte';
  import { ClipboardCheck, LayoutGrid, Clock3, Settings, Info, Pause, Play, Camera, LoaderCircle } from 'lucide-svelte';
  import ConnectionBadges from './ConnectionBadges.svelte';
  import type { GritImageInterpretationStatus } from '../../routes/grit/gritTypes';

  export let isRecording = true;
  export let isPaused = false;
  let count: number | null = null;
  let toggling = false;
  let error = '';

  let interpStatus: GritImageInterpretationStatus | null = null;
  let interpError = false;
  let pollingInterp = false;
  let nowSeconds = Math.floor(Date.now() / 1000);

  let countTimer: ReturnType<typeof setInterval> | null = null;
  let interpTimer: ReturnType<typeof setInterval> | null = null;
  let tickTimer: ReturnType<typeof setInterval> | null = null;

  const items = [
    { path: '/', label: 'Tidrapporter', icon: ClipboardCheck },
    { path: '/overview', label: 'Översikt', icon: LayoutGrid },
    { path: '/timeline', label: 'Tidslinje', icon: Clock3 },
    { path: '/settings', label: 'Inställningar', icon: Settings },
    { path: '/about', label: 'Om', icon: Info },
  ];

  function getLocalTodayIso(): string {
    const now = new Date();
    const y = now.getFullYear();
    const m = String(now.getMonth() + 1).padStart(2, '0');
    const d = String(now.getDate()).padStart(2, '0');
    return `${y}-${m}-${d}`;
  }

  function formatCountdown(seconds: number): string {
    const clamped = Math.max(0, Math.ceil(seconds));
    const mins = Math.floor(clamped / 60);
    const secs = clamped % 60;
    return `${mins}:${String(secs).padStart(2, '0')}`;
  }

  async function refreshCount() {
    try {
      const stats = await invoke<{ screenshot_count: number }>('get_today_stats');
      count = typeof stats?.screenshot_count === 'number' ? stats.screenshot_count : null;
    } catch {
      count = null;
    }
  }

  async function refreshInterpretationStatus() {
    if (pollingInterp) return;
    pollingInterp = true;
    try {
      interpStatus = await invoke<GritImageInterpretationStatus>('grit_interpretation_status', {
        date: getLocalTodayIso(),
      });
      interpError = false;
    } catch {
      interpError = true;
    } finally {
      pollingInterp = false;
    }
  }

  onMount(() => {
    void refreshCount();
    void refreshInterpretationStatus();
    countTimer = setInterval(refreshCount, 15000);
    interpTimer = setInterval(refreshInterpretationStatus, 3000);
    tickTimer = setInterval(() => {
      nowSeconds = Math.floor(Date.now() / 1000);
    }, 1000);
  });

  onDestroy(() => {
    if (countTimer !== null) clearInterval(countTimer);
    if (interpTimer !== null) clearInterval(interpTimer);
    if (tickTimer !== null) clearInterval(tickTimer);
  });

  async function toggleRecording() {
    toggling = true;
    error = '';
    try {
      await invoke(isPaused || !isRecording ? 'resume_recording' : 'pause_recording');
    } catch {
      error = 'Kunde inte ändra insamlingen. Försök igen.';
    } finally {
      toggling = false;
    }
  }

  $: collecting = isRecording && !isPaused;
  $: interpreting = Boolean(interpStatus?.running && !interpError);
  $: interpLabel = (() => {
    if (interpError) return 'Bildstatus otillgänglig';
    if (interpreting) return 'Sammanställer bilddata…';
    if (!interpStatus) return 'Laddar bildstatus…';
    if (interpStatus.mode === 'off') return 'Bildtolkning av';
    if (interpStatus.requires_consent) return 'Samtycke saknas';
    if (interpStatus.next_run_at !== null && interpStatus.next_run_at > nowSeconds) {
      const prefix = interpStatus.mode === 'ai' ? 'AI' : 'OCR';
      return `${prefix} om ${formatCountdown(interpStatus.next_run_at - nowSeconds)}`;
    }
    return 'Väntar på bearbetning';
  })();
</script>

<div class="grit-sidebar flex h-full min-h-0 flex-col overflow-y-auto">
  <div class="grit-brand">
    <img src="/icons/256x256.png" alt="" width="32" height="32" style="image-rendering: pixelated;" />
    <strong>GRIT Work</strong>
  </div>
  <div class="capture-state">
    <div class="flex items-center justify-between gap-2">
      <span class="flex items-center gap-2 text-xs">
        <LoaderCircle
          size={14}
          class={collecting ? 'sidebar-spin text-emerald-600' : 'text-slate-400'}
          aria-hidden="true"
        />
        <span>{isPaused ? 'Pausad' : isRecording ? 'Samlar in' : 'Inaktiv'}</span>
      </span>
      <button
        class="ui-icon-button"
        disabled={toggling}
        on:click={toggleRecording}
        aria-label={isPaused || !isRecording ? 'Återuppta' : 'Pausa'}
        title={isPaused || !isRecording ? 'Återuppta' : 'Pausa'}
      >
        <svelte:component this={isPaused || !isRecording ? Play : Pause} size={15} />
      </button>
    </div>
    <p class="mt-2 flex items-center gap-1.5 text-xs text-slate-500">
      <Camera size={13} />
      <span class="font-mono">{count ?? '—'}</span> bilder idag
    </p>
    <p class="mt-1.5 flex items-center gap-1.5 text-[11px] leading-snug text-slate-500">
      {#if interpreting}
        <LoaderCircle size={12} class="sidebar-spin shrink-0 text-amber-500" aria-hidden="true" />
      {/if}
      <span class="break-words">{interpLabel}</span>
    </p>
    {#if error}
      <p role="alert" class="mt-2 text-xs text-red-700">{error}</p>
    {/if}
  </div>
  <nav aria-label="Huvudnavigation" class="grit-main-nav mb-6">
    {#each items as item}
      {@const active = item.path === '/' ? $location === '/' || $location === '/report' : $location.startsWith(item.path)}
      <a href={item.path} use:link aria-current={active ? 'page' : undefined} title={item.label}>
        <svelte:component this={item.icon} size={17} />
        <span>{item.label}</span>
      </a>
    {/each}
  </nav>
  <ConnectionBadges />
</div>

<style>
  @keyframes sidebar-rotate {
    to {
      transform: rotate(360deg);
    }
  }

  :global(.sidebar-spin) {
    animation: sidebar-rotate 1.1s linear infinite;
  }

  @media (prefers-reduced-motion: reduce) {
    :global(.sidebar-spin) {
      animation: none;
    }
  }
</style>
