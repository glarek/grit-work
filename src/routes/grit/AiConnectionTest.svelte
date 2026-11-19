<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { CheckCircle2, CircleAlert, LoaderCircle, PlugZap } from 'lucide-svelte';
  import { notifyGritChanged } from './workflow.ts';
  import HelpTip from '$lib/components/ui/HelpTip.svelte';
  export let target: 'report' | 'image' = 'report';
  export let configKey = '';
  export let disabled = false;
  let testing = false;
  let previousKey = configKey;
  let result: { ok: boolean; message: string } | null = null;
  $: if (previousKey !== configKey) { previousKey = configKey; result = null; }
  async function test() {
    const key = configKey;
    testing = true;
    result = null;
    try {
      const response = await invoke<{ ok: boolean; message: string }>('grit_test_ai', { target });
      if (configKey === key) result = response;
    } catch (error) {
      if (configKey === key) result = { ok: false, message: typeof error === 'string' ? error : 'Testet kunde inte köras. Försök igen.' };
    } finally { testing = false; notifyGritChanged(); }
  }
</script>

<div class="space-y-2">
  <div class="flex items-center gap-2">
    <button class="ui-button" disabled={disabled || testing} on:click={test}>
      {#if testing}<LoaderCircle size={15} class="animate-spin" />{:else}<PlugZap size={15} />{/if}
      {testing ? 'Testar…' : target === 'image' ? 'Testa bildmodell' : 'Testa AI'}
    </button>
    <HelpTip text="Testar sparad leverantör och modell med syntetiska testdata. Inga aktivitetsloggar eller arbetsbilder skickas. Bildtestet kontrollerar att modellen faktiskt kan läsa text i en testbild. Kan ta upp till 90 sekunder." />
    {#if disabled}<span class="text-xs text-slate-500">Spara ändringarna först</span>{/if}
  </div>
  {#if result}
    <p role="status" class="flex items-start gap-1.5 text-xs {result.ok ? 'text-emerald-700' : 'text-amber-800'}">
      <svelte:component this={result.ok ? CheckCircle2 : CircleAlert} size={15} class="mt-px shrink-0" />
      <span>{result.message}</span>
    </p>
  {/if}
</div>
