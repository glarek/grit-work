<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { AlertCircle, AlertTriangle, CheckCircle2, Clock3 } from 'lucide-svelte';
  import { Tooltip } from 'bits-ui';
  import type { GritProvider } from '../../routes/grit/gritTypes';
  export let provider: GritProvider | null = null;
  $: providerName = provider === 'codex' ? 'Codex' : provider === 'grok' ? 'Grok' : provider === 'agy' ? 'Antigravity' : 'AI';
  $: providerIcon = provider ? `/icons/connections/${provider}.${provider === 'grok' ? 'png' : 'svg'}` : null;

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

<div class="connection-icons" data-connection-badges aria-label="Anslutningar">
  <Tooltip.Root openDelay={150}>
    <Tooltip.Trigger class="connection-icon" aria-label={stateLabel(providerName, aiAggregateState, unavailable)}>
      {#if providerIcon}<img class:monochrome={provider === 'codex'} src={providerIcon} alt={providerName} width="22" height="22" />{:else}<span class="connection-fallback">AI</span>{/if}
      <span class="connection-health" data-state={unavailable ? 'untested' : aiAggregateState}><svelte:component this={stateIcon(aiAggregateState, unavailable)} size={12} /></span>
    </Tooltip.Trigger>
    <Tooltip.Content sideOffset={8} class="ui-tooltip">{aiTooltip}</Tooltip.Content>
  </Tooltip.Root>
  <Tooltip.Root openDelay={150}>
    <Tooltip.Trigger class="connection-icon" aria-label={stateLabel('Spiris', spirisHealth.state, unavailable)}>
      <img src="/icons/connections/spiris.png" alt="Spiris" width="23" height="23" />
      <span class="connection-health" data-state={unavailable ? 'untested' : spirisHealth.state}><svelte:component this={stateIcon(spirisHealth.state, unavailable)} size={12} /></span>
    </Tooltip.Trigger>
    <Tooltip.Content sideOffset={8} class="ui-tooltip">{spirisTooltip}</Tooltip.Content>
  </Tooltip.Root>
</div>
