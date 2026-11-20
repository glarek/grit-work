<script lang="ts">
  import { onMount } from 'svelte';
  import { location, link } from 'svelte-spa-router';
  import { Settings, ChevronDown, Moon, Sun, UserRound } from 'lucide-svelte';
  import ConnectionBadges from './ConnectionBadges.svelte';
  import UserPicker from './UserPicker.svelte';
  import UserAvatar from './UserAvatar.svelte';
  import { gritAccount, refreshGritAccount } from '../stores/gritAccount';
  import { dashboardTheme, setDashboardTheme } from '../stores/appearance';

  let pickerOpen = false;
  $: status = $gritAccount.status;
  $: user = status?.users.find(person => person.id === status?.settings.user_id);
  $: overview = $location === '/' || $location === '/overview';
  $: heading = overview ? (user ? `Hej, ${user.name.split(' ')[0]}!` : 'Din arbetsöversikt')
    : $location.startsWith('/settings') ? 'Inställningar' : $location.startsWith('/timeline') ? 'Tidslinje'
    : $location.startsWith('/about') ? 'Om GRIT Work' : 'Tidrapporter';

  onMount(() => {
    const refresh = () => { void refreshGritAccount(); };
    refresh();
    window.addEventListener('grit-changed', refresh);
    const timer = setInterval(() => { if (!document.hidden) refresh(); }, 15000);
    return () => { clearInterval(timer); window.removeEventListener('grit-changed', refresh); };
  });
</script>

<header class="workspace-toolbar">
  <div class="workspace-heading"><h1>{heading}</h1><p>{overview ? 'Din arbetsdag, samlad på ett ställe.' : 'Arbetsyta för din tid och dina projekt.'}</p></div>
  <div class="workspace-tools">
    <ConnectionBadges provider={status?.settings.provider ?? null} />
    <button class="workspace-icon-button" aria-label={$dashboardTheme === 'dark' ? 'Byt till ljust tema' : 'Byt till mörkt tema'} title={$dashboardTheme === 'dark' ? 'Ljust tema' : 'Mörkt tema'} on:click={() => setDashboardTheme($dashboardTheme === 'dark' ? 'light' : 'dark')}><svelte:component this={$dashboardTheme === 'dark' ? Sun : Moon} size={19} /></button>
    <a href="/settings" use:link class="workspace-icon-button" aria-label="Inställningar" title="Inställningar" aria-current={$location === '/settings' ? 'page' : undefined}><Settings size={20} /></a>
    <button class="workspace-profile" aria-label="Välj Spiris-användare" aria-haspopup="dialog" on:click={() => pickerOpen = true}>
      {#if user}<UserAvatar id={user.id} />{:else}<span class="user-avatar-placeholder"><UserRound size={22} /></span>{/if}
      <span class="workspace-profile-copy"><strong>{user?.name ?? 'Välj användare'}</strong><small>{$gritAccount.error ? 'Status otillgänglig' : 'Spiris-användare'}</small></span><ChevronDown size={15} />
    </button>
  </div>
</header>
<UserPicker bind:open={pickerOpen} />
