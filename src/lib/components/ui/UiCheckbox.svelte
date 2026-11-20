<script lang="ts">
  import { Checkbox } from 'bits-ui';
  import { Check } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';
  export let checked=false;
  export let id: string | undefined = undefined;
  export let disabled=false;
  export let label:string;
  const dispatch=createEventDispatcher<{change:boolean}>();
</script>
<Checkbox.Root {id} {checked} {disabled} aria-label={label} class="ui-checkbox" onCheckedChange={next=>{checked=next===true;dispatch('change',checked);}}>
  <Checkbox.Indicator><Check size={14} strokeWidth={2.5}/></Checkbox.Indicator>
</Checkbox.Root>
