<script lang="ts">
  import { createSelect } from '@melt-ui/svelte';
  import { ChevronDown, Check } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';
  export let value='';
  export let id:string|undefined=undefined;
  export let options:{value:string;label:string;disabled?:boolean}[]=[];
  export let label='';
  export let placeholder='Välj…';
  export let disabled=false;
  const dispatch=createEventDispatcher<{change:string}>();
  const select=createSelect<string>({
    positioning:{placement:'bottom-start',sameWidth:true,fitViewport:true,gutter:5},
    onSelectedChange:({next})=>{
      if(next && next.value!==value){value=next.value;dispatch('change',value);}
      return next;
    }
  });
  const {elements:{trigger,menu,option},states:{selected,open},options:{disabled:disabledOption},ids}=select;
  $: selectedOption=options.find(o=>o.value===String(value));
  $: $selected=selectedOption;
  $: $disabledOption=disabled;
  $: if(id) ids.trigger.set(id);
</script>
<button {...$trigger} use:$trigger.action type="button" class="ui-select-trigger" aria-label={label || undefined}>
  <span class="truncate">{selectedOption?.label || placeholder}</span><ChevronDown size={16} aria-hidden="true" />
</button>
{#if $open}
  <div {...$menu} use:$menu.action class="ui-select-content">
    {#each options as item}
      <div {...$option(item)} use:$option.action class="ui-select-item">
        <span>{item.label}</span>{#if item.value===String(value)}<Check size={15} aria-hidden="true" />{/if}
      </div>
    {/each}
  </div>
{/if}
