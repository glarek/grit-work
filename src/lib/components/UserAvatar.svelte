<script lang="ts">
  export let id = 0;
  // Stable illustrations generated from a catalogue ID, never inferred portraits.
  const palettes = [
    ['#e5d8ed', '#765092', '#43333c'],
    ['#dce8df', '#517965', '#54402f'],
    ['#e0e8f1', '#4b6f8c', '#393244'],
    ['#f1dfd8', '#b2715b', '#976646'],
  ];
  $: palette = palettes[Math.abs(id) % palettes.length];
</script>

<svg class="user-avatar" viewBox="0 0 80 80" fill="none" aria-hidden="true">
  <circle cx="40" cy="40" r="40" fill={palette[0]} />
  <path d="M8 78c2-18 14-26 32-26s30 8 32 26" fill={palette[1]} />
  <path d="M33 46v12c4 6 10 6 14 0V46" fill="#e5b99c" />
  <ellipse cx="40" cy="33" rx="17" ry="21" fill="#edc5a7" />
  <path d="M23 32c-5-23 12-28 24-24 13 1 18 17 10 27l-3-16c-8 5-16 1-21 5l-9 7" fill={palette[2]} />
  <circle cx="34" cy="34" r="1.2" fill="#3c2a29" /><circle cx="46" cy="34" r="1.2" fill="#3c2a29" />
  <path d="M36 43q4 3 8 0" stroke="#9e6756" stroke-width="1.5" stroke-linecap="round" />
  {#if id % 3 === 0}<path d="M27 33h11v6H27zm15 0h11v6H42zm-4 2h4" stroke="#443d3f" stroke-width="1.2" />{/if}
</svg>
