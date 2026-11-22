<script lang="ts">
  import { confirmDialog, resolveConfirm } from '$lib/stores/confirm.ts';
  import UiDialog from './ui/UiDialog.svelte';
  import { TriangleAlert } from 'lucide-svelte';
  let open = false;
  $: dialogState = $confirmDialog;
  $: if (dialogState) open = true;
  $: if (!open && dialogState) resolveConfirm(false);
</script>
{#if dialogState}
  <UiDialog bind:open title={dialogState.title} description={dialogState.message}>
    <div class="flex items-center justify-between gap-4">
      <TriangleAlert size={24} class="text-amber-600" />
      <div class="flex flex-wrap gap-3">
        <button class="ui-button" on:click={() => resolveConfirm(false)}>{dialogState.cancelText}</button>
        <button class="ui-button ui-button-primary" on:click={() => resolveConfirm(true)}>{dialogState.confirmText}</button>
      </div>
    </div>
  </UiDialog>
{/if}
