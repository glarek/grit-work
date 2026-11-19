<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import GritIcon from './GritIcon.svelte';
  import { selectWorkDate } from './workflow.ts';
  import { getStatusBadgeMeta } from './gritTypes.ts';

  export let date: string;
  export const current: string = '';
  export let archived = false;
  export let showProjects = false;

  type DayContext = {
    source: string;
    activities: number;
    minutes: number;
    report_status: string | null;
    projects?: { id: number; name: string; minutes: number; needs_review: boolean }[];
  };

  let context: DayContext | null = null;
  let error = '';
  let sequence = 0;
  let mounted = false;
  let lastDate = '';
  let poll: ReturnType<typeof setInterval>;

  async function refresh() {
    const request = ++sequence;
    error = '';
    try {
      const value = await invoke<DayContext>('grit_day_context', { date });
      if (request === sequence) {
        context = value;
        archived = value?.source === 'archive' || value?.source === 'mixed';
      }
    } catch (e) {
      if (request === sequence) error = String(e);
    }
  }

  $: if (mounted && date !== lastDate) {
    context = null;
    lastDate = date;
    void refresh();
  }

  onMount(() => {
    mounted = true;
    window.addEventListener('grit-changed', refresh);
    poll = setInterval(refresh, 15000);
  });

  onDestroy(() => {
    sequence++;
    clearInterval(poll);
    if (typeof window !== 'undefined') window.removeEventListener('grit-changed', refresh);
  });

  const links: Array<{ path: string; label: string; icon: 'overview' | 'timeline' | 'report' }> = [
    { path: '/overview', label: 'Översikt', icon: 'overview' },
    { path: '/timeline', label: 'Tidslinje', icon: 'timeline' },
    { path: '/report', label: 'Tidrapport', icon: 'report' },
  ];

  $: reportStatusLabel = context?.report_status
    ? getStatusBadgeMeta(context.report_status).label
    : 'Inte skapad';
</script>

<div class="grit-workflow mb-5 rounded-xl border border-slate-200/90 bg-slate-50/90 p-3.5 text-sm dark:border-[#30363d] dark:bg-[#161b22]">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div class="flex flex-wrap items-center gap-2.5">
      <strong class="font-mono text-sm font-semibold text-slate-900 dark:text-[#e6edf3]">{date}</strong>
      <span class="text-xs text-slate-500 dark:text-[#8b949e]">
        {#if context}
          {context.activities} poster · {Math.round(context.minutes)} min · {context.source === 'archive' ? 'Arkiv' : context.source === 'mixed' ? 'Lokalt och arkiv' : 'Lokalt'}
        {:else}
          {error || 'Hämtar underlag…'}
        {/if}
      </span>
    </div>

    <span class="inline-flex items-center gap-1.5 rounded-full border border-slate-200 bg-white px-2.5 py-0.5 text-xs font-medium text-slate-700 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#adbac7]">
      <GritIcon name="report" sizeClass="h-3.5 w-3.5 text-indigo-600 dark:text-indigo-400" />
      <span>{reportStatusLabel}</span>
    </span>
  </div>



  {#if showProjects}
    <div class="mt-3.5 border-t border-slate-200 pt-3 dark:border-[#30363d]">
      <div class="flex flex-wrap items-baseline justify-between gap-2">
        <h3 class="text-xs font-semibold text-slate-800 dark:text-[#e6edf3]">Projekt i sparad rapport</h3>
        <span class="text-[11px] text-slate-500 dark:text-[#8b949e]">Debiterbar tid fastställs vid granskning.</span>
      </div>
      {#if context?.projects?.length}
        <div class="mt-2.5 flex flex-wrap gap-2">
          {#each context.projects as project}
            <a
              href="#/report"
              on:click={() => selectWorkDate(date)}
              class="inline-flex items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs transition hover:border-indigo-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 dark:border-[#30363d] dark:bg-[#0d1117] dark:hover:border-indigo-700"
            >
              <span class="text-slate-700 dark:text-[#adbac7]">{project.name}</span>
              <strong class="font-mono text-slate-900 dark:text-[#e6edf3]">{project.minutes} min</strong>
              {#if project.needs_review}
                <GritIcon name="alert" sizeClass="h-3.5 w-3.5 text-amber-600 dark:text-amber-400" />
              {/if}
            </a>
          {/each}
        </div>
      {:else}
        <p class="mt-1.5 text-xs text-slate-500 dark:text-[#8b949e]">Ingen sparad projektfördelning ännu.</p>
      {/if}
    </div>
  {/if}
</div>
