<script lang="ts">
  import { toast, clearToast, type ToastType } from '$lib/stores/toast.ts';

  import { Check, X, TriangleAlert, Info } from 'lucide-svelte';
  const iconMap = {success:Check,error:X,warning:TriangleAlert,info:Info};

  // Alger Music 式语义 tint 玻璃：半透明主题色底 + 亮暗各自前景 + 毛玻璃
  const colorMap: Record<ToastType, string> = {
    success: 'bg-[rgba(34,197,94,0.15)] dark:bg-[rgba(34,197,94,0.18)] text-[#16a34a] dark:text-[#4ade80]',
    error: 'bg-[rgba(239,68,68,0.12)] dark:bg-[rgba(239,68,68,0.18)] text-[#dc2626] dark:text-[#f87171]',
    warning: 'bg-[rgba(245,158,11,0.12)] dark:bg-[rgba(245,158,11,0.18)] text-[#d97706] dark:text-[#fbbf24]',
    info: 'bg-[rgba(59,130,246,0.12)] dark:bg-[rgba(59,130,246,0.18)] text-[#2563eb] dark:text-[#60a5fa]',
  };

  $: toastState = $toast;
  $: icon = iconMap[toastState?.type ?? 'info'];
  $: toastClass = colorMap[toastState?.type ?? 'info'];
</script>

{#if toastState}
  <div
    class="fixed inset-x-0 bottom-6 z-[210] flex justify-center px-4 animate-fadeIn pointer-events-none"
    role="status"
    aria-live="polite"
  >
    <button
      type="button"
      on:click={clearToast}
      class={`toast-glass max-w-xl min-h-11 px-[18px] py-2.5 text-sm font-medium flex items-center gap-2 pointer-events-auto ${toastClass}`}
    >
      <svelte:component this={icon} size={16} class="shrink-0" />
      <span class="leading-snug text-center break-words min-w-0">{toastState.message}</span>
    </button>
  </div>
{/if}
