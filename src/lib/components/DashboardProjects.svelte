<script lang="ts">
  import { ArrowUpRight } from 'lucide-svelte';
  import type { DayContext } from '../../routes/grit/dayContext';
  import { selectWorkDate } from '../../routes/grit/workflow';
  import { formatDurationLocalized } from '$lib/i18n';
  export let context: DayContext | null = null;
  export let error = '';
  export let date: string;
  const colors = ['#b477d4', '#d9b4ed', '#865a9e', '#67cba6', '#80a5db', '#d4aa74'];
  $: projects = (context?.projects ?? []).filter(project => Number.isFinite(project.minutes) && project.minutes > 0);
  $: total = projects.reduce((sum, project) => sum + project.minutes, 0);
  $: segments = projects.map((project, i) => ({...project, color: colors[i % colors.length]}));
  $: gradient = (() => {
    let start = 0;
    return segments.map(project => { const end = start + project.minutes / total * 100; const stop = `${project.color} ${start}% ${end}%`; start = end; return stop; }).join(', ');
  })();
</script>

<section class="dashboard-projects" aria-busy={!context && !error}>
  <div class="dashboard-card-heading"><h2>Projektfördelning</h2><a href="#/report" on:click={() => selectWorkDate(date)} class="workspace-icon-button" aria-label="Granska projekt i tidrapporten"><ArrowUpRight size={18} /></a></div>
  <p class="dashboard-muted">Sparad tidrapport · {date}</p>
  {#if error}<p class="dashboard-error" role="status">Projektfördelningen kunde inte hämtas.</p>
  {:else if !context}<p class="dashboard-empty">Hämtar projekt…</p>
  {:else if total > 0}
    <div class="project-donut" style={`--donut: conic-gradient(${gradient})`} role="img" aria-label={`Sparad projekttid: ${formatDurationLocalized(total * 60, { compact: true })}. Fördelning visas nedan.`}><div><strong>{formatDurationLocalized(total * 60, { compact: true })}</strong><span>sparad tid</span></div></div>
    <ul class="project-legend">{#each segments as project}<li><span class="project-dot" style={`background:${project.color}`}></span><span class="project-name">{project.name}{#if project.needs_review}<small>Behöver granskas</small>{/if}</span><strong>{formatDurationLocalized(project.minutes * 60, { compact: true })}</strong></li>{/each}</ul>
    <p class="dashboard-footnote">Debiterbar tid fastställs vid granskning.</p>
  {:else}<div class="dashboard-empty"><div class="project-donut project-donut-empty" aria-hidden="true"><div><strong>—</strong><span>sparad tid</span></div></div><p>Projektfördelningen visas när du har sparat en tidrapport.</p></div>{/if}
</section>
