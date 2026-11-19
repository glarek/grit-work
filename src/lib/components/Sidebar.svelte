<script lang="ts">
  import { location, link } from 'svelte-spa-router';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { ClipboardCheck, LayoutGrid, Clock3, ChartNoAxesColumn, Settings, Info, Pause, Play, Camera } from 'lucide-svelte';
  export let isRecording = true;
  export let isPaused = false;
  let count: number | null = null;
  let toggling = false;
  let error = '';
  const items = [
    {path:'/',label:'Tidrapporter',icon:ClipboardCheck},
    {path:'/overview',label:'Översikt',icon:LayoutGrid},
    {path:'/timeline',label:'Tidslinje',icon:Clock3},
    {path:'/ask',label:'Tidsanalys',icon:ChartNoAxesColumn},
    {path:'/settings',label:'Inställningar',icon:Settings},
    {path:'/about',label:'Om',icon:Info},
  ];
  async function refreshCount() {
    try { const stats = await invoke<{screenshot_count:number}>('get_today_stats'); count = typeof stats?.screenshot_count === 'number' ? stats.screenshot_count : null; } catch { count = null; }
  }
  onMount(() => { void refreshCount(); const timer=setInterval(refreshCount,15000); return()=>clearInterval(timer); });
  async function toggleRecording() {
    toggling=true;error='';
    try { await invoke(isPaused || !isRecording ? 'resume_recording' : 'pause_recording'); }
    catch { error='Kunde inte ändra insamlingen. Försök igen.'; }
    finally { toggling=false; }
  }
</script>
<div class="grit-sidebar">
  <div class="grit-brand"><img src="/icons/256x256.png" alt="" width="30" height="30"/><strong>GRIT Work</strong></div>
  <div class="capture-state">
    <div class="flex items-center justify-between gap-2">
      <span class="flex items-center gap-2 text-xs"><span class:collecting={isRecording && !isPaused} class="capture-dot"></span>{isPaused ? 'Pausad' : isRecording ? 'Samlar in' : 'Inaktiv'}</span>
      <button class="ui-icon-button" disabled={toggling} on:click={toggleRecording} aria-label={isPaused || !isRecording ? 'Återuppta' : 'Pausa'} title={isPaused || !isRecording ? 'Återuppta' : 'Pausa'}><svelte:component this={isPaused || !isRecording ? Play : Pause} size={15}/></button>
    </div>
    <p class="flex items-center gap-1.5 text-xs text-slate-500 mt-2"><Camera size={13}/><span class="font-mono">{count ?? '—'}</span> bilder idag</p>
    {#if error}<p role="alert" class="text-xs text-red-700 mt-2">{error}</p>{/if}
  </div>
  <nav aria-label="Huvudnavigation" class="grit-main-nav">
    {#each items as item}
      {@const active = item.path==='/' ? $location==='/' || $location==='/report' : $location.startsWith(item.path)}
      <a href={item.path} use:link aria-current={active ? 'page' : undefined} title={item.label}><svelte:component this={item.icon} size={17}/><span>{item.label}</span></a>
    {/each}
  </nav>
</div>
