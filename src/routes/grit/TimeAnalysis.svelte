<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { invoke } from '@tauri-apps/api/core';
  import WorkflowBar from './WorkflowBar.svelte';
  import LocalizedDatePicker from '$lib/components/LocalizedDatePicker.svelte';
  import { workDate, selectWorkDate, notifyGritChanged } from './workflow.ts';
  import { normalizeGritStatus, PROVIDER_LABELS } from './gritTypes.ts';
  export let params: {date?: string} = {};
  type Answer = {question:string; answer:string; provider:string; model:string; created_at:number};
  let date = params.date || get(workDate);
  let question = '';
  let history: Answer[] = [];
  let busy = false;
  let submittingAnalysis = false;
  let error = '';
  let provider = '';
  let model = '';
  let aiConsent = false;
  let request = 0;
  let runningDate: string | null = null;
  let poll: ReturnType<typeof setInterval>;
  let disposed = false;
  const now = new Date();
  const today = `${now.getFullYear()}-${String(now.getMonth()+1).padStart(2,'0')}-${String(now.getDate()).padStart(2,'0')}`;
  async function load() {
    selectWorkDate(date); error = ''; history = [];
    const id = ++request;
    try {
      const [answers, raw] = await Promise.all([invoke<Answer[]>('grit_get_analysis',{date}),invoke('grit_status')]);
      if (id !== request) return;
      history = Array.isArray(answers) ? answers : [];
      const status = normalizeGritStatus(raw); provider = PROVIDER_LABELS[status.settings.provider].name; model = status.settings.model; aiConsent = status.settings.ai_consent;
    } catch(e) { if(id === request) error = String(e); }
  }
  async function analyze() {
    if (!question.trim() || busy || !aiConsent) return;
    submittingAnalysis = true; busy = true; runningDate = date; error = '';
    try { history = await invoke<Answer[]>('grit_analyze',{date,question}); question = ''; notifyGritChanged(); }
    catch(e) { error = String(e); }
    finally { submittingAnalysis = false; busy = false; runningDate = null; }
  }
  async function cancel() { try { await invoke('grit_cancel'); } catch(e) { error=String(e); } }
  async function pollJob() {
    try {
      const active = await invoke<string | null>('grit_analysis_job');
      if (disposed) return;
      runningDate = active;
      if (busy && !active && !submittingAnalysis) {
        const answers = await invoke<Answer[]>('grit_get_analysis',{date});
        if (!disposed) { history = answers; busy = false; }
      } else if (active) busy = true;
    } catch { /* The running invocation displays its own errors. */ }
  }
  onMount(() => { void load(); void pollJob(); poll = setInterval(pollJob,3000); });
  onDestroy(() => { disposed = true; request++; clearInterval(poll); });
</script>
<div class="grit-work analysis-view mx-auto max-w-6xl space-y-5 px-4 py-5 sm:px-6 sm:py-6">
  <header class="grit-header flex flex-wrap items-end justify-between gap-4">
    <div><h1 class="text-slate-900">Tidsanalys</h1><p class="mt-1 text-sm text-slate-500">Förstå arbetet bakom tidrapporten.</p></div>
    <div class="flex flex-wrap items-center gap-2 text-sm text-slate-600">
      <span>Arbetsdag</span>
      <LocalizedDatePicker
        bind:value={date}
        max={today}
        disabled={busy}
        label="Välj analysdag"
        on:change={load}
      />
    </div>
  </header>
  <WorkflowBar {date} current="/ask" />
  <section class="rounded-xl border border-slate-200 bg-white p-4 sm:p-5 dark:border-[#30363d] dark:bg-[#161b22]" aria-label="Analysera arbetsdagen">
    <div class="mb-4 flex flex-wrap items-center justify-between gap-2 text-sm">
      <span class="inline-flex items-center gap-2 font-semibold text-slate-700 dark:text-[#adbac7]">
        <svg class="h-4 w-4 text-indigo-600 dark:text-indigo-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M4 19h16M6 15v-4m6 4V5m6 10V8" stroke-linecap="round"/></svg>
        {provider || 'Vald AI-leverantör'}
      </span>
      <a class="text-slate-500 underline underline-offset-4 dark:text-[#8b949e]" href="#/settings" title="Ändra AI-leverantör, modell och samtycke">AI-inställningar</a>
    </div>
    {#if !aiConsent}
      <p class="mb-4 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900 dark:border-amber-800/60 dark:bg-amber-950/30 dark:text-amber-200">
        Godkänn delning av underlag under <a class="font-semibold underline" href="#/settings">Inställningar</a> för att starta analysen.
      </p>
    {/if}
    <label class="block text-sm font-semibold text-slate-700 dark:text-[#adbac7]">Fråga om arbetsdagen<textarea bind:value={question} maxlength="4000" disabled={busy} rows="3" placeholder="Vad vill du veta om dagens arbete?" aria-describedby="analysis-sharing" class="mt-2 w-full resize-y rounded-lg border border-slate-200 bg-transparent p-3 font-normal leading-relaxed dark:border-[#30363d]" /></label>
    <div class="mt-2 flex flex-wrap gap-2" aria-label="Förslag på frågor">
      {#each [
        {label:'Projektfördelning', prompt:'Vilket arbete hör till respektive projekt?', path:'M3 7h7l2 2h9v11H3zM3 7V4h7l2 3'},
        {label:'Luckor och osäker tid', prompt:'Var finns luckor eller osäker tid?', path:'M12 8v5m0 3h.01M12 3 2 21h20L12 3Z'},
        {label:'Sammanfatta', prompt:'Sammanfatta underlaget inför min tidrapport.', path:'M5 3h14v18H5zM8 8h8M8 12h8M8 16h5'}
      ] as suggestion}
        <button disabled={busy} on:click={() => question=suggestion.prompt} title={suggestion.prompt} aria-label={suggestion.prompt} class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 px-3 py-2 text-xs text-slate-600 hover:bg-slate-50 disabled:opacity-50 dark:border-[#30363d] dark:text-[#adbac7] dark:hover:bg-[#21262d]">
          <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" aria-hidden="true"><path d={suggestion.path}/></svg>{suggestion.label}
        </button>
      {/each}
    </div>
    <p id="analysis-sharing" class="mt-4 max-w-prose text-xs leading-relaxed text-slate-500 dark:text-[#8b949e]">När du startar skickas dagens bilder, aktivitetsloggar och projektunderlag till {provider || 'vald AI-leverantör'}. Ingen tidrapport ändras och ingen tid skickas till Spiris.</p>
    <div class="mt-4 flex flex-wrap items-center gap-3">
      <button disabled={busy || !question.trim() || !aiConsent} on:click={analyze} class="inline-flex items-center gap-2 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700 disabled:opacity-50">
        <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="m5 3 15 9-15 9V3Z" stroke-linejoin="round"/></svg>
        {busy ? 'Analyserar i bakgrunden…' : 'Starta analys'}
      </button>
      {#if busy}<button on:click={cancel} class="inline-flex items-center gap-2 rounded-lg border border-slate-200 px-4 py-2 text-sm dark:border-[#30363d] dark:text-[#adbac7]"><svg class="h-3 w-3" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><rect x="3" y="3" width="10" height="10" rx="1"/></svg>Avbryt analys</button>{/if}
    </div>
    {#if runningDate}<p role="status" class="mt-3 text-sm text-slate-500 dark:text-[#8b949e]">Bakgrundsanalys pågår för {runningDate}. Du kan lämna vyn.</p>{/if}
    {#if error}<p role="alert" class="mt-3 text-sm text-rose-600 dark:text-rose-400">{error}</p>{/if}
    <details class="mt-4 border-t border-slate-100 pt-3 text-xs text-slate-500 dark:border-[#30363d] dark:text-[#8b949e]">
      <summary class="cursor-pointer py-1">Om analysen</summary>
      <p class="mt-2 max-w-prose leading-relaxed">Modell: <span class="break-all font-mono">{model || 'Inte vald'}</span>. Analysen körs via leverantörens inloggade CLI. Kontrollera slutsatser mot tidslinjen.</p>
    </details>
  </section>
  {#if history.length}
    <section aria-labelledby="analysis-history" class="space-y-3">
      <h2 id="analysis-history" class="text-sm font-semibold text-slate-600 dark:text-[#adbac7]">Tidigare analyser <span class="ml-1 font-mono text-xs font-normal text-slate-500">{history.length}</span></h2>
      {#each [...history].reverse() as entry}
        <article class="rounded-xl border border-slate-200 bg-white p-4 sm:p-5 dark:border-[#30363d] dark:bg-[#161b22]">
          <div class="flex flex-wrap items-baseline justify-between gap-2">
            <h3 class="font-semibold text-slate-900 dark:text-[#e6edf3]">{entry.question}</h3>
            <time datetime={new Date(entry.created_at*1000).toISOString()} class="font-mono text-xs text-slate-500 dark:text-[#8b949e]">{new Date(entry.created_at*1000).toLocaleString('sv-SE')}</time>
          </div>
          <p class="mt-3 max-w-prose whitespace-pre-wrap break-words text-sm leading-7 text-slate-700 dark:text-[#adbac7]">{entry.answer}</p>
          <details class="mt-3 text-xs text-slate-500 dark:text-[#8b949e]"><summary class="cursor-pointer py-1">AI-modell</summary><p class="mt-1 break-all">{entry.provider}: <span class="font-mono">{entry.model}</span></p></details>
        </article>
      {/each}
    </section>
  {/if}
</div>

<style>
  .analysis-view :is(button, a, input, textarea, summary):focus-visible {
    outline: 2px solid #6366f1;
    outline-offset: 3px;
  }
</style>
