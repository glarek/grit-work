<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { selectWorkDate } from './workflow.ts';
  import { getStatusBadgeMeta } from './gritTypes.ts';
  export let date: string;
  export let current = '';
  export let archived = false;
  export let showProjects = false;
  type DayContext = { source: string; activities: number; minutes: number; report_status: string | null; projects?: {id:number;name:string;minutes:number;needs_review:boolean}[] };
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
      if (request === sequence) { context = value; archived = value?.source === 'archive'; }
    } catch (e) { if (request === sequence) error = String(e); }
  }
  $: if (mounted && date !== lastDate) { context = null; lastDate = date; void refresh(); }
  onMount(() => { mounted = true; window.addEventListener('grit-changed', refresh); poll = setInterval(refresh, 15000); });
  onDestroy(() => { sequence++; clearInterval(poll); if (typeof window !== 'undefined') window.removeEventListener('grit-changed', refresh); });
  const links = [{path:'/overview',label:'Översikt'}, {path:'/timeline',label:'Tidslinje'}, {path:'/ask',label:'Analysera'}, {path:'/report',label:'Tidrapport'}];
</script>
<div class="grit-workflow mb-5 rounded-xl border border-slate-200 bg-slate-50 p-4 text-sm dark:border-[#30363d] dark:bg-[#161b22]">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div><strong class="font-mono text-slate-900 dark:text-[#e6edf3]">{date}</strong>
      <span class="ml-3 text-slate-500 dark:text-[#8b949e]">{context ? `${context.activities} poster · ${Math.round(context.minutes)} min · ${context.source === 'archive' ? 'verifierat arkiv' : 'lokalt underlag'}` : error || 'Hämtar gemensamt underlag…'}</span>
    </div>
    <span class="text-xs text-slate-600 dark:text-[#adbac7]">Rapport: {context?.report_status ? getStatusBadgeMeta(context.report_status).label : 'inte skapad'}</span>
  </div>
  <nav aria-label="Arbetsflöde för vald dag" class="mt-3 flex flex-wrap gap-2">
    {#each links as item}<a href={`#${item.path}`} on:click={() => selectWorkDate(date)} aria-current={current === item.path ? 'page' : undefined} class="rounded-lg border border-slate-200 px-3 py-1.5 text-slate-700 hover:bg-white dark:border-[#30363d] dark:text-[#adbac7] dark:hover:bg-[#21262d]">{item.label}</a>{/each}
  </nav>
  {#if showProjects}
    <div class="mt-4 border-t border-slate-200 pt-3 dark:border-[#30363d]">
      <h3 class="font-semibold text-slate-800 dark:text-[#e6edf3]">Tidrapportens projektfördelning</h3>
      <p class="mt-1 text-xs text-slate-500">Sparade rapportrader för denna dag. Registrerad aktivitet ovan är underlag; debiterbar tid fastställs vid granskningen.</p>
      {#if context?.projects?.length}<div class="mt-3 flex flex-wrap gap-3">{#each context.projects as project}<a href="#/report" on:click={() => selectWorkDate(date)} class="rounded-lg border border-slate-200 bg-white px-3 py-2 dark:border-[#30363d] dark:bg-[#0d1117]"><span class="text-slate-700 dark:text-[#adbac7]">{project.name}</span> <strong class="ml-2 font-mono text-slate-900 dark:text-[#e6edf3]">{project.minutes} min</strong>{#if project.needs_review}<span class="ml-2 text-xs text-amber-600">Kräver granskning</span>{/if}</a>{/each}</div>
      {:else}<p class="mt-2 text-sm text-slate-500">Ingen sparad projektfördelning ännu. Skapa och granska ett rapportutkast.</p>{/if}
    </div>
  {/if}
</div>
