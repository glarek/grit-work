<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { Check, ArrowRight, RefreshCw, Search } from 'lucide-svelte';
  import { push } from 'svelte-spa-router';
  import UiDialog from './ui/UiDialog.svelte';
  import UserAvatar from './UserAvatar.svelte';
  import { gritAccount, refreshGritAccount } from '../stores/gritAccount';
  import { notifyGritChanged } from '../../routes/grit/workflow';
  import { showToast } from '../stores/toast';

  export let open = false;
  let wasOpen = false;
  let selectedId = 0;
  let query = '';
  let loading = false;
  let saving = false;
  let error = '';
  $: status = $gritAccount.status;
  $: users = [...(status?.users ?? [])].sort((a, b) => a.name.localeCompare(b.name, 'sv'));
  $: filteredUsers = users.filter(user => user.name.toLocaleLowerCase('sv').includes(query.trim().toLocaleLowerCase('sv')));
  $: selectedUser = users.find(user => user.id === selectedId);
  $: {
    if (open && !wasOpen) { query = ''; error = ''; selectedId = status?.settings.user_id ?? 0; void load(false); }
    wasOpen = open;
  }

  async function load(sync: boolean): Promise<void> {
    loading = true;
    error = '';
    try {
      if (sync) await invoke('grit_sync_spiris');
      const next = await refreshGritAccount();
      if (!next) throw new Error('Användarna kunde inte hämtas. Försök igen.');
      if (!next.users.some(user => user.id === selectedId)) selectedId = next.settings.user_id;
      if (sync) notifyGritChanged();
    } catch (e) { error = typeof e === 'string' ? e : e instanceof Error ? e.message : 'Synkroniseringen misslyckades.'; }
    finally { loading = false; }
  }

  async function chooseUser(): Promise<void> {
    if (!selectedUser || saving) return;
    const chosen = selectedUser;
    if (chosen.id === status?.settings.user_id) { open = false; return; }
    saving = true;
    error = '';
    try {
      if (chosen.id !== status?.settings.user_id && !window.dispatchEvent(new CustomEvent('grit-user-changing', { cancelable: true }))) {
        throw new Error('Spara dina ändringar i tidrapporten och avsluta pågående rapportarbete innan du byter användare.');
      }
      // A partial backend update preserves concurrent edits to AI and capture settings.
      await invoke('grit_select_user', { userId: chosen.id });
      window.dispatchEvent(new CustomEvent('grit-user-changed', { detail: { userId: chosen.id } }));
      notifyGritChanged();
      const saved = await refreshGritAccount();
      if (!saved || saved.settings.user_id !== chosen.id) throw new Error('Valet kunde inte läsas tillbaka. Uppdatera användarlistan innan du fortsätter.');
      showToast(`${chosen.name} är vald för tidrapportering.`, 'success');
      open = false;
    } catch (e) { error = typeof e === 'string' ? e : e instanceof Error ? e.message : 'Användaren kunde inte väljas.'; }
    finally { saving = false; }
  }
</script>

<UiDialog bind:open title="Vem arbetar idag?" description="Välj den Spiris-användare som du vill rapportera tid för." dismissible={!saving} contentClass="user-picker-dialog">
  <div class="user-picker-source"><img src="/icons/connections/spiris.png" alt="" width="20" height="20" /><span>Användare från Spiris</span>
    <button class="ui-icon-button" aria-label="Uppdatera användare från Spiris" title="Uppdatera användare från Spiris" disabled={loading || saving || !status?.credential_saved} on:click={() => load(true)}><RefreshCw size={16} class={loading ? 'animate-spin' : ''} /></button>
  </div>
  {#if error || $gritAccount.error}<p class="dashboard-error" role="alert">{error || $gritAccount.error}</p>{/if}
  {#if !status?.credential_saved && !loading}
    <div class="dashboard-empty"><p>Anslut Spiris i inställningarna för att hämta dina användare.</p><button class="ui-button ui-button-primary" on:click={() => { open = false; push('/settings'); }}>Öppna inställningar</button></div>
  {:else}
    <label class="user-search"><Search size={17} aria-hidden="true" /><input type="search" bind:value={query} placeholder="Sök efter en användare" aria-label="Sök Spiris-användare" disabled={saving} /></label>
    <div class="user-picker-grid" aria-busy={loading} role="group" aria-label="Spiris-användare">
      {#each filteredUsers as user (user.id)}
        <button class="user-choice" class:selected={selectedId === user.id} aria-pressed={selectedId === user.id} disabled={saving || loading} on:click={() => selectedId = user.id}>
          <UserAvatar id={user.id} /><span><strong>{user.name}</strong><small>{user.id === status?.settings.user_id ? 'Vald användare' : 'Spiris-användare'}</small></span>
          {#if selectedId === user.id}<Check class="user-choice-check" size={18} aria-hidden="true" />{/if}
        </button>
      {/each}
    </div>
    {#if loading}<p class="dashboard-muted" role="status">Hämtar användare…</p>
    {:else if !users.length}<div class="dashboard-empty"><p>Inga användare har synkroniserats ännu.</p><button class="ui-button" disabled={saving || !status?.credential_saved} on:click={() => load(true)}>Hämta användare från Spiris</button></div>
    {:else if !filteredUsers.length}<p class="dashboard-muted">Ingen användare matchar sökningen.</p>{/if}
    <div class="user-picker-actions"><p>Valet används när du granskar och rapporterar din tid.</p><button class="ui-button ui-button-primary" disabled={saving || loading || !selectedUser} on:click={chooseUser}>{saving ? 'Sparar…' : selectedUser ? `Fortsätt som ${selectedUser.name}` : 'Välj en användare'}<ArrowRight size={16} /></button></div>
  {/if}
</UiDialog>
