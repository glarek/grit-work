<script lang="ts">
  import { Tooltip } from 'bits-ui';
  import { Info } from 'lucide-svelte';
  export let text = '';
  export let label = 'Mer information';
</script>
<Tooltip.Root openDelay={150}>
  <Tooltip.Trigger type="button" class="ui-help" aria-label={label}><Info size={15} aria-hidden="true" /></Tooltip.Trigger>
  <Tooltip.Content sideOffset={7} class="ui-tooltip">{text}<slot /></Tooltip.Content>
</Tooltip.Root>
