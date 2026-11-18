<script lang="ts">
  export let name:
    | 'overview'
    | 'timeline'
    | 'analyze'
    | 'report'
    | 'settings'
    | 'sync'
    | 'archive'
    | 'sparkles'
    | 'plus'
    | 'check'
    | 'alert'
    | 'clock'
    | 'chevron-left'
    | 'chevron-right'
    | 'chevron-down'
    | 'folder'
    | 'key'
    | 'shield'
    | 'trash'
    | 'download'
    | 'cancel';

  export let sizeClass = 'h-4 w-4';
</script>

<svg
  class="{sizeClass} shrink-0"
  fill="none"
  stroke="currentColor"
  viewBox="0 0 24 24"
  aria-hidden="true"
>
  {#if name === 'overview'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
  {:else if name === 'timeline'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
  {:else if name === 'analyze'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M8 10h8M8 14h5m-7 6h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
  {:else if name === 'report'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4" />
  {:else if name === 'settings'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
  {:else if name === 'sync'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
  {:else if name === 'archive'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4" />
  {:else if name === 'sparkles'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M12 3l1.9 4.6L18.5 9.5l-4.6 1.9L12 16l-1.9-4.6L5.5 9.5l4.6-1.9L12 3z" />
  {:else if name === 'plus'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M12 5v14m-7-7h14" />
  {:else if name === 'check'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M5 13l4 4L19 7" />
  {:else if name === 'alert'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M12 9v4m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
  {:else if name === 'clock'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M12 6v6l4 2m6-2a10 10 0 11-20 0 10 10 0 0120 0z" />
  {:else if name === 'chevron-left'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M15 19l-7-7 7-7" />
  {:else if name === 'chevron-right'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M9 5l7 7-7 7" />
  {:else if name === 'chevron-down'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M6 9l6 6 6-6" />
  {:else if name === 'folder'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
  {:else if name === 'key'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M15 7a4 4 0 11-7.75 1.37L3 12.62V16h3.38l1.24-1.24H10v-2.38l1.12-1.12A4 4 0 0115 7z" />
  {:else if name === 'shield'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M12 3l7 4v5c0 4.5-3 8.4-7 9.5C8 20.4 5 16.5 5 12V7l7-4z" />
  {:else if name === 'trash'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
  {:else if name === 'download'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2M7 10l5 5 5-5M12 15V3" />
  {:else if name === 'cancel'}
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M6 18L18 6M6 6l12 12" />
  {/if}
</svg>
