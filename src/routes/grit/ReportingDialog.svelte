<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { RefreshCw, Send, History, TriangleAlert, ChevronDown } from 'lucide-svelte';
  import UiDialog from '../../lib/components/ui/UiDialog.svelte';
  import UiSelect from '../../lib/components/ui/UiSelect.svelte';
  import HelpTip from '../../lib/components/ui/HelpTip.svelte';
  import { computeProjectRounding, formatMinutesSwedish, formatSwedishHours, validateReportForSubmit, type GritReport, type GritStatus, type GritChoice } from './gritTypes';
  import { reportingChoicesValid, type ReportingContext } from './reportingContext';
  export let open = true;
  export let report: GritReport;
  export let status: GritStatus;
  export let submitting = false;
  const dispatch = createEventDispatcher<{change:GritReport;submit:void}>();
  let context: ReportingContext | null = null;
  let loading = true;
  let error = '';
  let historyOpen = true;
  async function load() {
    loading = true; error = ''; context = null;
    try { context = await invoke<ReportingContext>('grit_reporting_context', {date:report.date}); }
    catch (err) { error = typeof err === 'string' ? err : 'Uppgifterna från Spiris kunde inte hämtas. Försök igen.'; }
    finally { loading = false; }
  }
  function choose(index:number, key:'activity_id'|'time_code_id'|'article_id', value:string) {
    report = {...report, rows:report.rows.map((row,i) => i === index ? {...row,[key]:Number(value)} : row)};
    dispatch('change', report);
  }
  const options = (choices:GritChoice[] = []) => choices.map(c => ({value:String(c.id),label:c.name}));
  $: rounding = computeProjectRounding(report.rows, status.projects);
  $: valid = !loading && !error && reportingChoicesValid(report, context) && validateReportForSubmit(report,status).canSubmit;
  $: history = context?.previous_rows ?? [];
  onMount(load);
</script>
<UiDialog bind:open title="Rapportera till Spiris" description="Välj aktivitet, tidskod och artikel. Granska sedan hela rapporten innan du skickar." dismissible={!submitting}>
  <div class="reporting-dialog">
    <div class="reporting-summary">
      <span class="font-mono">{report.date}</span>
      <span>{context?.user.name ?? 'Hämtar användare…'}</span>
      <strong>{formatMinutesSwedish(rounding.totalSubmittedMinutes)} till Spiris</strong>
    </div>
    {#if loading}
      <p role="status" class="flex items-center gap-2 py-8 text-sm text-slate-500"><RefreshCw size={16} class="animate-spin" />Hämtar val och tidigare rapportering…</p>
    {:else if error}
      <div role="alert" class="reporting-warning"><p>{error}</p><button class="ui-button" on:click={load}><RefreshCw size={15} />Försök igen</button></div>
    {:else if context}
      {#if context.existing_rows.length}
        <div role="alert" class="reporting-warning"><TriangleAlert size={18} /><p>Det finns redan {context.existing_rows.length} rapporterade rader för denna dag. Överföringen är spärrad för att undvika dubbletter. Kontrollera raderna i Spiris.</p></div>
        <div class="reporting-table-wrap"><table><caption>Redan rapporterat {report.date}</caption><thead><tr><th>Projekt</th><th>Aktivitet / tidskod / artikel</th><th>Tid</th><th>Beskrivning</th></tr></thead><tbody>{#each context.existing_rows as row}<tr><td>{row.project?.name ?? '—'}</td><td>{row.activity?.name ?? '—'} / {row.time_code?.name ?? '—'} / {row.article?.name ?? '—'}</td><td>{formatSwedishHours(row.hours)}</td><td>{row.comment}</td></tr>{/each}</tbody></table></div>
      {/if}
      <div class="flex items-center justify-between gap-3 text-xs text-slate-500">
        <span class="inline-flex items-center gap-1">Aktiva val från Spiris <HelpTip text="Vi kontrollerar projektets status, ditt medlemskap och aktiva registerposter. Spiris API visar inga projektspecifika begränsningar för aktivitet, tidskod eller artikel. Läsåtkomst bevisar inte skrivrättighet. Valen kontrolleras på nytt precis innan överföring." /></span>
        <button class="ui-icon-button" aria-label="Uppdatera val från Spiris" disabled={submitting} on:click={load}><RefreshCw size={15} /></button>
      </div>
      {#each report.rows as row,index (row.id)}
        {@const project = context.projects.find(p => p.project_id === row.project_id)}
        <section class="reporting-row" aria-label={`Rapporteringsrad ${index + 1}`}>
          <div class="flex items-start justify-between gap-4"><h3 class="font-semibold text-sm">{status.projects.find(p => p.id === row.project_id)?.name ?? `Projekt #${row.project_id}`}</h3><span class="font-mono text-xs whitespace-nowrap">{formatMinutesSwedish(rounding.rowSubmittedMinutes[index] ?? row.minutes)}</span></div>
          <p class="text-sm text-slate-600">{row.description}</p>
          {#if !project?.member || !project?.active}<p class="text-sm text-rose-700" role="alert">Projektet är inaktivt eller du saknar medlemskap. Överföringen är spärrad.</p>{/if}
          <div class="reporting-choices">
            <div><label for={`report-activity-${index}`}> Aktivitet</label><UiSelect id={`report-activity-${index}`} label={`Aktivitet, rad ${index+1}`} value={String(row.activity_id)} options={options(project?.activities)} disabled={submitting || !project?.member || !project?.active} on:change={(e)=>choose(index,'activity_id',e.detail)} /></div>
            <div><label for={`report-time-${index}`}> Tidskod</label><UiSelect id={`report-time-${index}`} label={`Tidskod, rad ${index+1}`} value={String(row.time_code_id ?? 0)} options={options(project?.time_codes)} disabled={submitting || !project?.member || !project?.active} on:change={(e)=>choose(index,'time_code_id',e.detail)} /></div>
            <div><label for={`report-article-${index}`}> Artikel</label><UiSelect id={`report-article-${index}`} label={`Artikel, rad ${index+1}`} value={String(row.article_id)} options={options(project?.articles)} disabled={submitting || !project?.member || !project?.active} on:change={(e)=>choose(index,'article_id',e.detail)} /></div>
          </div>
        </section>
      {/each}
      <div class="reporting-history">
        <button class="reporting-history-toggle" aria-expanded={historyOpen} on:click={()=>historyOpen=!historyOpen}><History size={16} /><span>Tidigare rapportering i dessa projekt</span><span class="text-slate-400">{history.length}</span><ChevronDown size={16} class={historyOpen ? 'rotate-180' : ''} /></button>
        {#if historyOpen}<div class="reporting-history-content">
          {#if history.length}<div class="reporting-table-wrap"><table><caption>Senaste 30 raderna från dina föregående 90 dagar</caption><thead><tr><th>Datum / projekt</th><th>Aktivitet</th><th>Tidskod / artikel</th><th>Tid</th><th>Beskrivning</th></tr></thead><tbody>{#each history as row}<tr><td><span class="font-mono whitespace-nowrap">{row.date.slice(0,10)}</span><br />{row.project?.name ?? '—'}</td><td>{row.activity?.name ?? '—'}</td><td>{row.time_code?.name ?? '—'}<br /><span class="text-slate-500">{row.article?.name ?? '—'}</span></td><td class="whitespace-nowrap">{formatSwedishHours(row.hours)}</td><td>{row.comment}</td></tr>{/each}</tbody></table></div>{:else}<p class="text-sm text-slate-500 py-3">Ingen tidigare rapportering hittades i dessa projekt under de föregående 90 dagarna.</p>{/if}
        </div>{/if}
      </div>
    {/if}
    <div class="reporting-totals">
      {#each rounding.projectTotals as project}<div class="flex justify-between gap-3"><span>{project.projectName}</span><span class="font-mono whitespace-nowrap">{formatMinutesSwedish(project.recordedMinutes)} → {formatMinutesSwedish(project.submittedMinutes)}</span></div>{/each}
      <p class="text-xs text-slate-500">Avrundat uppåt per projekt och dygn till 30 minuter. Fakturerbart: {formatSwedishHours(status.settings.billable ? rounding.totalSubmittedMinutes/60 : 0)}.</p>
    </div>
    <p class="text-xs text-slate-500">Hela rapporten skickas vid godkännande och låses därefter för redigering.</p>
    <div class="flex flex-wrap justify-end gap-2"><button class="ui-button" disabled={submitting} on:click={()=>open=false}>Avbryt</button><button class="ui-button ui-button-primary" disabled={!valid || submitting} on:click={()=>dispatch('submit')}><Send size={16} />{submitting ? 'Skickar…' : 'Godkänn och skicka'}</button></div>
  </div>
</UiDialog>
<style>
  .reporting-dialog{display:grid;gap:18px}.reporting-summary{display:flex;flex-wrap:wrap;gap:12px 24px;align-items:center;padding:14px 16px;background:#f5f7fa;border-radius:var(--radius-md);font-size:13px}.reporting-summary strong{margin-left:auto}.reporting-row{display:grid;gap:9px;padding:16px;background:#f5f7fa;border-radius:var(--radius-lg);}.reporting-choices{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px}.reporting-choices>div{display:grid;gap:6px;font-size:12px;color:#475569}.reporting-history-toggle{display:flex;align-items:center;gap:8px;width:100%;padding:12px 0;font-size:13px;font-weight:600}.reporting-history-toggle span:first-of-type{flex:1;text-align:left}.reporting-history-content{animation:grit-popover .16s ease-out}.reporting-table-wrap{overflow:auto;max-height:300px}table{width:100%;font-size:12px;text-align:left;border-collapse:collapse}caption{text-align:left;color:#64748b;padding:0 0 12px}th{font-weight:500;color:#64748b;background:white;position:sticky;top:0}td,th{padding:9px 10px;vertical-align:top}tbody tr:nth-child(odd){background:#f5f7fa}.reporting-warning{display:flex;flex-wrap:wrap;align-items:center;gap:10px;padding:12px;background:#fff1f2;color:#9f1239;border-radius:var(--radius-md);font-size:13px}.reporting-totals{display:grid;gap:8px;font-size:13px}@media(max-width:640px){.reporting-choices{grid-template-columns:1fr}.reporting-summary strong{margin-left:0}}@media(prefers-reduced-motion:reduce){.reporting-history-content{animation:none}}
</style>
