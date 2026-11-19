<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { createDialog } from '@melt-ui/svelte';
  import { X } from 'lucide-svelte';
  export let open = false;
  export let title: string;
  export let description = '';
  export let dismissible = true;
  let focusBeforeOpen: HTMLElement | null = null;
  let wasOpen = false;
  async function restoreFocus() {
    await tick();
    if (focusBeforeOpen?.isConnected && !document.querySelector('[data-melt-dialog-content]')) focusBeforeOpen.focus();
  }
  $: {
    if (open && !wasOpen && typeof document !== 'undefined') focusBeforeOpen = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!open && wasOpen) void restoreFocus();
    wasOpen = open;
  }
  onDestroy(() => { if (wasOpen) void restoreFocus(); });
  // Use the same accessible primitives as Bits UI directly so forwarded slot
  // updates remain reactive in Svelte 4, including approval button states.
  const dialog = createDialog({defaultOpen:open,onOpenChange:({next})=>{open=next;return next;}});
  const {elements:{overlay,content,title:heading,description:descriptionElement,close,portalled},states:{open:dialogOpen},options:{closeOnEscape,closeOnOutsideClick}} = dialog;
  $: $dialogOpen = open;
  $: $closeOnEscape = dismissible;
  $: $closeOnOutsideClick = dismissible;
</script>
{#if open}
  <div {...$portalled} use:$portalled.action>
    <div {...$overlay} use:$overlay.action class="ui-dialog-overlay"></div>
    <div {...$content} use:$content.action class="ui-dialog-content">
      <div class="ui-dialog-heading"><h2 {...$heading} use:$heading.action class="text-xl font-semibold">{title}</h2>
        {#if dismissible}<button {...$close} use:$close.action aria-label="Stäng" class="ui-icon-button"><X size={18} /></button>{/if}
      </div>
      <p {...$descriptionElement} use:$descriptionElement.action class={description ? 'text-sm text-slate-500 mb-5' : 'sr-only'}>{description || title}</p>
      <slot />
    </div>
  </div>
{/if}
