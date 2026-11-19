<script lang="ts">
  import { Clock3, Crosshair, Globe, LayoutGrid } from 'lucide-svelte';

  export let title = '';
  export let value = '';
  export let icon = '';
  type StatsCardColor = 'indigo' | 'emerald' | 'amber' | 'rose' | 'blue' | 'violet';

  export let color: StatsCardColor = 'indigo';
  export let href: string | null = null;
  export let embedded = false;
  export let subtitle: string | null = null;
  /** 标记为概览 KPI 卡：供 ui-style-c 紧凑样式精准命中，避免误伤 subtitle */
  export let dataKpiCard = false;

  const iconColors: Record<StatsCardColor, string> = {
    indigo: 'text-indigo-600 dark:text-indigo-400',
    emerald: 'text-emerald-600 dark:text-emerald-400',
    amber: 'text-amber-600 dark:text-amber-400',
    rose: 'text-rose-600 dark:text-rose-400',
    blue: 'text-blue-600 dark:text-blue-400',
    violet: 'text-violet-600 dark:text-violet-400',
  };

  const iconBgs: Record<StatsCardColor, string> = {
    indigo: 'bg-indigo-50 dark:bg-indigo-950/40',
    emerald: 'bg-emerald-50 dark:bg-emerald-950/40',
    amber: 'bg-amber-50 dark:bg-amber-950/40',
    rose: 'bg-rose-50 dark:bg-rose-950/40',
    blue: 'bg-blue-50 dark:bg-blue-950/40',
    violet: 'bg-violet-50 dark:bg-violet-950/40',
  };

  const iconComponents: Record<string, typeof Clock3> = {
    duration: Clock3,
    focus: Crosshair,
    browser: Globe,
    apps: LayoutGrid,
  };

  $: iconComponent = iconComponents[icon] ?? null;
  $: cardClass = embedded
    ? 'block rounded-lg bg-slate-50/88 px-4 py-3.5 dark:bg-[#161b22]/30'
    : 'block rounded-lg border border-slate-200/80 bg-white p-4 dark:border-[#30363d]/60 dark:bg-[#21262d]/80';
  $: iconClass = `flex h-7 w-7 shrink-0 items-center justify-center rounded-lg ${iconBgs[color]} ${iconColors[color]}`;
  $: valueClass = 'mt-2.5 text-2xl font-semibold tabular-nums leading-tight text-slate-900 dark:text-[#e6edf3]';
  $: interactiveClass = embedded
    ? 'transition-colors duration-150 hover:bg-slate-100/90 dark:hover:bg-slate-900/50'
    : 'transition-colors duration-150 hover:border-slate-300 hover:bg-slate-50/70 dark:hover:border-[#484f58] cursor-pointer';
</script>

{#if href}
  <a
    {href}
    class="{cardClass} {interactiveClass}"
    data-kpi-card={dataKpiCard ? '' : undefined}
  >
    <div class="flex items-center justify-between gap-2">
      <span class="min-w-0 text-xs font-medium leading-snug text-slate-500 dark:text-[#7d8590]">{title}</span>
      {#if iconComponent}
        <div class={iconClass} aria-hidden="true">
          <svelte:component this={iconComponent} size={15} strokeWidth={1.8} />
        </div>
      {/if}
    </div>
    <p class={valueClass}>{value}</p>
    {#if subtitle}
      <p class="mt-1 text-xs font-medium leading-snug text-slate-500 dark:text-[#7d8590]">{subtitle}</p>
    {/if}
  </a>
{:else}
  <div class={cardClass} data-kpi-card={dataKpiCard ? '' : undefined}>
    <div class="flex items-center justify-between gap-2">
      <span class="min-w-0 text-xs font-medium leading-snug text-slate-500 dark:text-[#7d8590]">{title}</span>
      {#if iconComponent}
        <div class={iconClass} aria-hidden="true">
          <svelte:component this={iconComponent} size={15} strokeWidth={1.8} />
        </div>
      {/if}
    </div>
    <p class={valueClass}>{value}</p>
    {#if subtitle}
      <p class="mt-1 text-xs font-medium leading-snug text-slate-500 dark:text-[#7d8590]">{subtitle}</p>
    {/if}
  </div>
{/if}
