<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { invoke } from '@tauri-apps/api/core';
  import WorkflowBar from './WorkflowBar.svelte';
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
<div class="grit-work mx-auto max-w-6xl space-y-5 px-6 py-6">
  <header class="grit-header"><h1 class="text-slate-900 dark:text-[#e6edf3]">Tidsanalys</h1><p class="mt-2 text-sm text-slate-500 dark:text-[#8b949e]">Undersök arbetet bakom tidrapporten med dagens bilder, aktivitetsloggar och Spiris-projekt.</p></header>
  <label class="block text-sm text-slate-600 dark:text-[#adbac7]">Arbetsdag <input aria-label="Arbetsdag för analys" type="date" bind:value={date} max={today} disabled={busy} on:change={load} class="ml-3 rounded-lg border border-slate-200 bg-transparent px-3 py-2 font-mono dark:border-[#30363d]" /></label>
  <WorkflowBar {date} current="/ask" />
  <section class="rounded-xl border border-slate-200 bg-white p-5 dark:border-[#30363d] dark:bg-[#161b22]">
    <p class="mb-3 text-sm text-slate-500 dark:text-[#8b949e]">{provider} · <span class="font-mono">{model}</span> · <a class="underline" href="#/settings">Gemensamma inställningar</a></p>
    <p class="mb-4 text-sm text-slate-600 dark:text-[#adbac7]">När du startar analysen skickas valt underlag till din valda AI-leverantör via dess inloggade CLI. Analysen ändrar ingen tidrapport och skickar ingen tid till Spiris. Kontrollera slutsatser mot tidslinjen.</p>
    {#if !aiConsent}
      <p class="mb-4 rounded-lg border border-amber-200 bg-amber-50 p-3 text-xs text-amber-900 dark:border-amber-800/60 dark:bg-amber-950/30 dark:text-amber-200">
        Informerat samtycke krävs innan analys kan köras. Aktivera samtycke under <a class="font-semibold underline" href="#/settings">Inställningar</a>.
      </p>
    {/if}
    <div class="mb-3 flex flex-wrap gap-2">{#each ['Vilket arbete hör till respektive projekt?', 'Var finns luckor eller osäker tid?', 'Sammanfatta underlaget inför min tidrapport.'] as prompt}<button disabled={busy} on:click={() => question=prompt} class="rounded-lg border border-slate-200 px-3 py-2 text-xs text-slate-600 dark:border-[#30363d] dark:text-[#adbac7]">{prompt}</button>{/each}</div>
    <label class="block text-sm font-semibold text-slate-700 dark:text-[#adbac7]">Fråga om arbetsdagen<textarea bind:value={question} maxlength="4000" disabled={busy} rows="3" class="mt-2 w-full rounded-lg border border-slate-200 bg-transparent p-3 font-normal dark:border-[#30363d]" /></label>
    <div class="mt-3 flex gap-3"><button disabled={busy || !question.trim() || !aiConsent} on:click={analyze} class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white disabled:opacity-50">{busy ? 'Analyserar i bakgrunden…' : 'Starta analys'}</button>{#if busy}<button on:click={cancel} class="rounded-lg border px-4 py-2 text-sm dark:text-[#adbac7]">Avbryt analys</button>{/if}</div>
    {#if runningDate}<p class="mt-3 text-sm text-slate-500">Bakgrundsanalys pågår för {runningDate}. Du kan gå till andra vyer medan den körs.</p>{/if}
    {#if error}<p role="alert" class="mt-3 text-sm text-rose-600">{error}</p>{/if}
  </section>
  {#each [...history].reverse() as entry}
    <article class="rounded-xl border border-slate-200 bg-white p-5 dark:border-[#30363d] dark:bg-[#161b22]">
      <h2 class="font-semibold text-slate-900 dark:text-[#e6edf3]">{entry.question}</h2>
      <p class="mt-1 font-mono text-xs text-slate-500">{entry.provider} · {entry.model} · {new Date(entry.created_at*1000).toLocaleString('sv-SE')}</p>
      <p class="mt-4 whitespace-pre-wrap text-sm leading-7 text-slate-700 dark:text-[#adbac7]">{entry.answer}</p>
    </article>
  {/each}
</div>
