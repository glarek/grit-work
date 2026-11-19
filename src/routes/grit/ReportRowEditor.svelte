<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { Trash2, Check, ChevronDown } from 'lucide-svelte';
  import UiDialog from '../../lib/components/ui/UiDialog.svelte';
  import UiSelect from '../../lib/components/ui/UiSelect.svelte';
  import HelpTip from '../../lib/components/ui/HelpTip.svelte';
  import { timeInputToUnixSeconds, unixSecondsToTimeInput, type GritRow, type GritStatus } from './gritTypes';
  export let open=true;
  export let row:GritRow;
  export let rowIndex:number;
  export let date:string;
  export let status:GritStatus;
  export let readonly=false;
  const dispatch=createEventDispatcher<{save:{index:number;row:GritRow};delete:{index:number};close:void}>();
  let draft:GritRow={...row,evidence_ids:[...row.evidence_ids],time_code_id:row.time_code_id ?? 0};
  let startTime=unixSecondsToTimeInput(row.start);
  let endTime=unixSecondsToTimeInput(row.end);
  let hours=row.minutes/60;
  let error='';
  $: if(!open) dispatch('close');
  $: projectOptions=status.projects.map(p=>({value:String(p.id),label:`${p.number ? p.number+' · ' : ''}${p.name}`,disabled:!p.active}));
  const choices=(values:{id:number;name:string}[])=>values.map(c=>({value:String(c.id),label:c.name}));
  function projectChanged(value:string){draft={...draft,project_id:Number(value),activity_id:0,time_code_id:0,article_id:0};}
  function timeChanged(){
    const start=timeInputToUnixSeconds(date,startTime),end=timeInputToUnixSeconds(date,endTime);
    if(start>0 && end>start){draft={...draft,start,end,minutes:Math.floor((end-start)/60)};hours=draft.minutes/60;}
  }
  function hoursChanged(){
    if(Number.isFinite(hours)&&hours>0){
      const minutes=Math.round(hours*60);
      draft={...draft,minutes};
      if(draft.manual){draft.end=draft.start+minutes*60;endTime=unixSecondsToTimeInput(draft.end);}
    }
  }
  function save(){
    const start=timeInputToUnixSeconds(date,startTime),end=timeInputToUnixSeconds(date,endTime);
    if(!start||end<=start){error='Ange ett giltigt tidsintervall (HH:mm).';return;}
    if(!Number.isFinite(hours)||hours<=0||Math.round(hours*60)>(end-start)/60){error='Tiden måste vara positiv och rymmas inom intervallet.';return;}
    if(!draft.description.trim()){error='Skriv en dagboksanteckning.';return;}
    dispatch('save',{index:rowIndex,row:{...draft,start,end,minutes:Math.round(hours*60),needs_review:draft.project_id<=0}});open=false;
  }
</script>
<UiDialog bind:open title={readonly ? 'Rapporterad tidrad' : 'Redigera tidrad'} description={date}>
  <div class="space-y-5">
    {#if error}<p role="alert" class="text-sm text-rose-700">{error}</p>{/if}
    <div class="grid grid-cols-[minmax(0,1fr)_110px] gap-4">
      <div><label for="edit-project" class="block text-xs text-slate-500 mb-2">Projekt</label><UiSelect id="edit-project" label="Projekt" value={String(draft.project_id)} options={projectOptions} disabled={readonly} on:change={e=>projectChanged(e.detail)} /></div>
      <label class="block text-xs text-slate-500">Timmar<input aria-label="Timmar" type="number" min="0" step="0.5" bind:value={hours} on:change={hoursChanged} disabled={readonly} class="mt-2 w-full rounded-lg bg-slate-100 px-3 py-2.5 font-mono text-sm text-slate-900 border-0" /></label>
    </div>
    <label class="block text-xs text-slate-500">Dagbok<textarea aria-label="Dagbok" rows="4" bind:value={draft.description} disabled={readonly} class="mt-2 w-full resize-y rounded-lg bg-slate-100 px-3 py-3 text-sm leading-relaxed text-slate-900 border-0" placeholder="Beskriv arbetet kort och konkret."></textarea></label>
    <div>
      <div class="flex items-center gap-1 mb-2 text-xs text-slate-500">Rapporteringsval <HelpTip text="Välj från de synkroniserade Spiris-registren. Projektmedlemskap och aktiva val kontrolleras på nytt när rapporten skickas. Spiris API visar inga projektspecifika begränsningslistor." /></div>
      <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <div><label for="edit-activity" class="block text-xs text-slate-500 mb-1">Aktivitet</label><UiSelect id="edit-activity" label="Aktivitet" options={choices(status.activities)} value={String(draft.activity_id)} disabled={readonly} on:change={e=>draft={...draft,activity_id:Number(e.detail)}} /></div>
        <div><label for="edit-timecode" class="block text-xs text-slate-500 mb-1">Tidskod</label><UiSelect id="edit-timecode" label="Tidskod" options={choices(status.time_codes)} value={String(draft.time_code_id ?? 0)} disabled={readonly} on:change={e=>draft={...draft,time_code_id:Number(e.detail)}} /></div>
        <div><label for="edit-article" class="block text-xs text-slate-500 mb-1">Artikel</label><UiSelect id="edit-article" label="Artikel" options={choices(status.articles)} value={String(draft.article_id)} disabled={readonly} on:change={e=>draft={...draft,article_id:Number(e.detail)}} /></div>
      </div>
    </div>
    <details class="rounded-lg bg-slate-50 p-3"><summary class="flex justify-between items-center list-none cursor-pointer text-xs text-slate-600">Tidsintervall och underlag<ChevronDown size={16}/></summary>
      <div class="grid grid-cols-2 gap-4 mt-3">
        <label class="text-xs text-slate-500">Start<input type="text" inputmode="numeric" placeholder="HH:mm" bind:value={startTime} on:change={timeChanged} disabled={readonly} class="mt-1 w-full rounded-lg bg-white px-3 py-2 font-mono text-sm border-0" /></label>
        <label class="text-xs text-slate-500">Slut<input type="text" inputmode="numeric" placeholder="HH:mm" bind:value={endTime} on:change={timeChanged} disabled={readonly} class="mt-1 w-full rounded-lg bg-white px-3 py-2 font-mono text-sm border-0" /></label>
      </div>
      <p class="text-xs text-slate-500 mt-3">Underlag: {draft.evidence_ids.length ? draft.evidence_ids.map(id=>`#${id}`).join(', ') : 'Manuell tid'}</p>
    </details>
    <div class="flex items-center justify-between gap-3 pt-1">
      {#if !readonly}<button class="ui-icon-button text-rose-600" aria-label="Ta bort tidrad" title="Ta bort tidrad" on:click={()=>{dispatch('delete',{index:rowIndex});open=false;}}><Trash2 size={17}/></button>{:else}<span></span>{/if}
      <div class="flex gap-2"><button class="ui-button" on:click={()=>open=false}>{readonly ? 'Stäng' : 'Avbryt'}</button>{#if !readonly}<button class="ui-button ui-button-primary" on:click={save}><Check size={16}/>Spara rad</button>{/if}</div>
    </div>
  </div>
</UiDialog>
