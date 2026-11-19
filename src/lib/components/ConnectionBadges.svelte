<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { AlertCircle, AlertTriangle, CheckCircle2, Clock3 } from 'lucide-svelte';
  import HelpTip from '$lib/components/ui/HelpTip.svelte';

  type HealthState = 'ok' | 'error' | 'untested' | 'stale';

  interface Health {
    state: HealthState;
    checked_at: number | null;
    message: string;
  }

  interface ConnectionStatusResponse {
    spiris: Health;
    ai: Health;
    ai_image: Health | null;
  }

  let status: ConnectionStatusResponse | null = null;
  let ipcError = false;
  let inFlight = false;
  let browserOnline = typeof navigator !== 'undefined' ? navigator.onLine : true;
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  function normalizeHealth(raw: unknown): Health {
    if (!raw || typeof raw !== 'object') {
      return { state: 'untested', checked_at: null, message: '' };
    }
    const obj = raw as Record<string, unknown>;
    const rawState = obj.state;
    const state: HealthState =
      rawState === 'ok' || rawState === 'error' || rawState === 'untested' || rawState === 'stale'
        ? rawState
        : 'untested';
    const checked_at =
      typeof obj.checked_at === 'number' && Number.isFinite(obj.checked_at)
        ? obj.checked_at
        : null;
    const message = typeof obj.message === 'string' ? obj.message.trim() : '';
    return { state: state === 'ok' && (!checked_at || checked_at <= 0) ? 'untested' : state, checked_at, message };
  }

  function formatCheckedAt(unixSeconds: number | null): string {
    if (unixSeconds === null || !Number.isFinite(unixSeconds) || unixSeconds <= 0) {
      return 'Ej kontrollerad';
    }
    const d = new Date(unixSeconds * 1000);
    if (Number.isNaN(d.getTime())) return 'Ej kontrollerad';
    const yyyy = d.getFullYear();
    const mm = String(d.getMonth() + 1).padStart(2, '0');
    const dd = String(d.getDate()).padStart(2, '0');
    const hh = String(d.getHours()).padStart(2, '0');
    const min = String(d.getMinutes()).padStart(2, '0');
    return `${yyyy}-${mm}-${dd} ${hh}:${min}`;
  }

  function worstState(a: HealthState, b: HealthState): HealthState {
    const rank: Record<HealthState, number> = {
      error: 3,
      untested: 2,
      stale: 1,
      ok: 0,
    };
    return rank[a] >= rank[b] ? a : b;
  }

  function stateLabel(prefix: string, state: HealthState, unavailable: boolean): string {
    if (!browserOnline) return `${prefix} Offline`;
    if (unavailable) return `${prefix} Ej läst`;
    if (state === 'ok') return `${prefix} OK`;
    if (state === 'error') return `${prefix} Fel`;
    if (state === 'stale') return `${prefix} Inaktuell`;
    return `${prefix} Ej testad`;
  }

  function stateTextClass(state: HealthState, unavailable: boolean): string {
    if (unavailable) return 'text-slate-600';
    if (state === 'ok') return 'text-emerald-700';
    if (state === 'error') return 'text-rose-700';
    if (state === 'stale') return 'text-amber-700';
    return 'text-slate-600';
  }

  function stateIcon(state: HealthState, unavailable: boolean) {
    if (unavailable) return Clock3;
    if (state === 'ok') return CheckCircle2;
    if (state === 'error') return AlertCircle;
    if (state === 'stale') return AlertTriangle;
    return Clock3;
  }

  async function refresh() {
    if (inFlight) return;
    inFlight = true;
    try {
      const raw = await invoke<unknown>('grit_connection_status');
      if (!raw || typeof raw !== 'object') {
        ipcError = true;
        status = null;
        return;
      }
      const obj = raw as Record<string, unknown>;
      status = {
        spiris: normalizeHealth(obj.spiris),
        ai: normalizeHealth(obj.ai),
        ai_image: obj.ai_image && typeof obj.ai_image === 'object' ? normalizeHealth(obj.ai_image) : null,
      };
      ipcError = false;
    } catch {
      ipcError = true;
    } finally {
      inFlight = false;
    }
  }

  function handleNetworkChange() {
    browserOnline = typeof navigator !== 'undefined' ? navigator.onLine : true;
    void refresh();
  }

  function handleGritChanged() {
    void refresh();
  }

  onMount(() => {
    void refresh();
    pollTimer = setInterval(refresh, 15000);
    if (typeof window !== 'undefined') {
      window.addEventListener('grit-changed', handleGritChanged);
      window.addEventListener('online', handleNetworkChange);
      window.addEventListener('offline', handleNetworkChange);
    }
  });

  onDestroy(() => {
    if (pollTimer !== null) clearInterval(pollTimer);
    if (typeof window !== 'undefined') {
      window.removeEventListener('grit-changed', handleGritChanged);
      window.removeEventListener('online', handleNetworkChange);
      window.removeEventListener('offline', handleNetworkChange);
    }
  });

  $: offlineNote = !browserOnline ? ' Ingen nätanslutning just nu.' : '';
  $: unavailable = ipcError || !status || !browserOnline;

  $: spirisHealth = status?.spiris ?? { state: 'untested' as HealthState, checked_at: null, message: '' };
  $: aiReportHealth = status?.ai ?? { state: 'untested' as HealthState, checked_at: null, message: '' };
  $: aiImageHealth = status?.ai_image ?? null;
  $: aiAggregateState =
    aiImageHealth !== null ? worstState(aiReportHealth.state, aiImageHealth.state) : aiReportHealth.state;

  $: spirisTooltip = unavailable
    ? `Kunde inte läsa senaste Spiris-kontroll. Tester körs under Inställningar.${offlineNote}`
    : `${spirisHealth.message || 'Ingen kontroll utförd ännu.'} Senaste kontroll: ${formatCheckedAt(spirisHealth.checked_at)}. Visar senaste sparade kontroll (senaste testresultat). Tester körs under Inställningar.${offlineNote}`;

  $: aiTooltip = unavailable
    ? `Kunde inte läsa senaste AI-kontroll. Tester körs under Inställningar.${offlineNote}`
    : [
        `Tidrapport: ${aiReportHealth.message || 'Ej testad'} (${formatCheckedAt(aiReportHealth.checked_at)}).`,
        aiImageHealth
          ? `Bildtolkning: ${aiImageHealth.message || 'Ej testad'} (${formatCheckedAt(aiImageHealth.checked_at)}).`
          : '',
        'Visar senaste sparade kontroll (senaste testresultat). Tester körs under Inställningar.',
      ]
        .filter(Boolean)
        .join(' ') + offlineNote;
</script>

<div class="mt-auto pt-3" data-connection-badges>
  <div class="space-y-1.5 text-xs">
    <div class="flex items-center justify-between gap-1.5 rounded-lg bg-slate-100/80 px-2.5 py-1.5">
      <span class="inline-flex min-w-0 items-center gap-1.5 font-medium {stateTextClass(spirisHealth.state, unavailable)}">
        <svelte:component this={stateIcon(spirisHealth.state, unavailable)} size={14} class="shrink-0" aria-hidden="true" />
        <span class="truncate">{stateLabel('Spiris', spirisHealth.state, unavailable)}</span>
      </span>
      <HelpTip label="Om Spiris-status" text={spirisTooltip} />
    </div>

    <div class="flex items-center justify-between gap-1.5 rounded-lg bg-slate-100/80 px-2.5 py-1.5">
      <span class="inline-flex min-w-0 items-center gap-1.5 font-medium {stateTextClass(aiAggregateState, unavailable)}">
        <svelte:component this={stateIcon(aiAggregateState, unavailable)} size={14} class="shrink-0" aria-hidden="true" />
        <span class="truncate">{stateLabel('AI', aiAggregateState, unavailable)}</span>
      </span>
      <HelpTip label="Om AI-status" text={aiTooltip} />
    </div>
  </div>
</div>
