<script lang="ts">
  import { ArrowUpRight, ArrowRight, ClipboardCheck, Monitor, Globe, Clock3 } from 'lucide-svelte';
  import { formatDurationLocalized } from '$lib/i18n';
  import type { DayContext } from '../../routes/grit/dayContext';
  import { getStatusBadgeMeta } from '../../routes/grit/gritTypes';
  import { selectWorkDate } from '../../routes/grit/workflow';
  export let duration = 0;
  export let browserDuration = 0;
  export let appCount = 0;
  export let peak = '—';
  export let title = 'Aktiv tid';
  export let loading = false;
  export let date: string;
  export let context: DayContext | null = null;
  export let contextError = '';
  $: statusLabel = context?.report_status ? getStatusBadgeMeta(context.report_status).label : 'Inte skapad';
</script>

<div class="dashboard-summary">
  <section class="dashboard-hero" aria-busy={loading}>
    <div class="dashboard-hero-heading"><span>{title}</span><a href="#/timeline" on:click={() => selectWorkDate(date)} aria-label="Visa underlag i tidslinjen"><ArrowUpRight size={21} /></a></div>
    <strong class="dashboard-duration">{loading ? '—' : formatDurationLocalized(duration, { compact: true })}</strong>
    <p>Insamlad aktivitet vid datorn</p>
    <div class="dashboard-hero-facts">
      <span><Monitor size={16} /><b>{loading ? '—' : appCount}</b> appar</span>
      <span><Globe size={16} /><b>{loading ? '—' : formatDurationLocalized(browserDuration, { compact: true })}</b> webb</span>
      <span><Clock3 size={16} /><b>{loading ? '—' : peak}</b> mest aktiv</span>
    </div>
  </section>
  <section class="dashboard-review">
    <div class="dashboard-card-heading"><span class="dashboard-icon-tile"><ClipboardCheck size={21} /></span><span class="dashboard-status">{contextError ? 'Status otillgänglig' : context ? statusLabel : 'Hämtar…'}</span></div>
    <h2>Från aktivitet till tidrapport</h2>
    <p>Granska projekt och tid för {date} innan du godkänner rapporten.</p>
    <a class="dashboard-text-link" href="#/report" on:click={() => selectWorkDate(date)}>Öppna tidrapport <ArrowRight size={17} /></a>
  </section>
</div>
