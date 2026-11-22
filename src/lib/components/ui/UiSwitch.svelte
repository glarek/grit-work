<script lang="ts">
  import { Switch } from 'bits-ui';
  import { createEventDispatcher } from 'svelte';
  export let checked=false;
  export let disabled=false;
  export let label:string;
  const dispatch=createEventDispatcher<{change:boolean}>();
</script>
<Switch.Root bind:checked {disabled} aria-label={label} class="ui-switch" onCheckedChange={value=>dispatch('change',value)}>
  <Switch.Thumb class="ui-switch-thumb" />
</Switch.Root>
