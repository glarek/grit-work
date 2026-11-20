<script lang="ts">
import { Calendar, RangeCalendar, Popover } from 'bits-ui';
import { parseDate } from '@internationalized/date';
import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-svelte';
import { createEventDispatcher } from 'svelte';
export let mode: 'single' | 'range' = 'single';
export const localeCode: string = 'sv-SE';
export let value = ''; export let startDate = ''; export let endDate = '';
export let min = ''; export let max = ''; export let open = false;
export let hideTrigger = false; export const inlinePanel: boolean = false;
export let disabled = false; export let label = 'Välj datum';
export let triggerClass = 'page-control-input';
const dispatch = createEventDispatcher<{change:void}>();
// Bits UI 0.21 / Melt offsets weekdays from the locale first day: sv-SE + 0 = Monday.
function parsed(v:string) { try { return v ? parseDate(v) : undefined; } catch { return undefined; } }
$: selected = parsed(value);
$: range = {start:parsed(startDate),end:parsed(endDate)};
$: display = mode === 'range' ? `${startDate || 'Från'} – ${endDate || 'Till'}` : value || 'Välj datum';
function singleChanged(v:any) { if (v && v.toString() !== value) { value=v.toString(); open=false; dispatch('change'); } }
function rangeChanged(v:any) { if(v?.start && v?.end && (v.start.toString()!==startDate || v.end.toString()!==endDate)) {startDate=v.start.toString();endDate=v.end.toString();open=false;dispatch('change');} }
</script>
<Popover.Root bind:open>
{#if !hideTrigger}<Popover.Trigger class={triggerClass + ' ui-date-trigger'} {disabled} aria-label={label}><CalendarDays size={16}/><span>{display}</span></Popover.Trigger>{/if}
<Popover.Content class="ui-calendar-content" sideOffset={6}>
{#if mode === 'single'}
<Calendar.Root value={selected} onValueChange={singleChanged} locale="sv-SE" weekStartsOn={0} weekdayFormat="short" minValue={parsed(min)} maxValue={parsed(max)} fixedWeeks let:months let:weekdays>
<Calendar.Header class="ui-calendar-header"><Calendar.PrevButton aria-label="Föregående månad" class="ui-icon-button"><ChevronLeft size={16}/></Calendar.PrevButton><Calendar.Heading/><Calendar.NextButton aria-label="Nästa månad" class="ui-icon-button"><ChevronRight size={16}/></Calendar.NextButton></Calendar.Header>
{#each months as month}<Calendar.Grid class="ui-calendar-grid"><Calendar.GridHead><Calendar.GridRow>{#each weekdays as day}<Calendar.HeadCell>{day}</Calendar.HeadCell>{/each}</Calendar.GridRow></Calendar.GridHead><Calendar.GridBody>{#each month.weeks as week}<Calendar.GridRow>{#each week as date}<Calendar.Cell {date}><Calendar.Day {date} month={month.value} class="ui-calendar-day">{date.day}</Calendar.Day></Calendar.Cell>{/each}</Calendar.GridRow>{/each}</Calendar.GridBody></Calendar.Grid>{/each}
</Calendar.Root>
{:else}
<RangeCalendar.Root value={range} onValueChange={rangeChanged} locale="sv-SE" weekStartsOn={0} weekdayFormat="short" minValue={parsed(min)} maxValue={parsed(max)} fixedWeeks let:months let:weekdays>
<RangeCalendar.Header class="ui-calendar-header"><RangeCalendar.PrevButton aria-label="Föregående månad" class="ui-icon-button"><ChevronLeft size={16}/></RangeCalendar.PrevButton><RangeCalendar.Heading/><RangeCalendar.NextButton aria-label="Nästa månad" class="ui-icon-button"><ChevronRight size={16}/></RangeCalendar.NextButton></RangeCalendar.Header>
{#each months as month}<RangeCalendar.Grid class="ui-calendar-grid"><RangeCalendar.GridHead><RangeCalendar.GridRow>{#each weekdays as day}<RangeCalendar.HeadCell>{day}</RangeCalendar.HeadCell>{/each}</RangeCalendar.GridRow></RangeCalendar.GridHead><RangeCalendar.GridBody>{#each month.weeks as week}<RangeCalendar.GridRow>{#each week as date}<RangeCalendar.Cell {date}><RangeCalendar.Day {date} month={month.value} class="ui-calendar-day">{date.day}</RangeCalendar.Day></RangeCalendar.Cell>{/each}</RangeCalendar.GridRow>{/each}</RangeCalendar.GridBody></RangeCalendar.Grid>{/each}
</RangeCalendar.Root>
{/if}
</Popover.Content>
</Popover.Root>
