<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-shell';
  import { getVersion } from '@tauri-apps/api/app';
  import { locale, t } from '$lib/i18n/index.ts';
  import { Info, FolderOpen, ExternalLink, Shield, ListTree, CircleCheck } from 'lucide-svelte';

  const GRIT_WORK_REPO_URL = 'https://github.com/glarek/grit-work';
  const UPSTREAM_REPO_URL = 'https://github.com/wm94i/Work-Review';

  let appVersion = '';
  let runtimePlatform = '';
  $: currentLocale = $locale;

  onMount(async () => {
    try {
      const version = await getVersion();
      appVersion = typeof version === 'string' && version.trim() ? version : '--';
    } catch (e) {
      console.error('Kunde inte läsa appversion:', e);
      appVersion = '--';
    }

    try {
      const platform = await invoke<string>('get_runtime_platform');
      runtimePlatform = typeof platform === 'string' && platform.trim() ? platform : '--';
    } catch (e) {
      console.error('Kunde inte läsa plattform:', e);
      runtimePlatform = '--';
    }
  });

  async function openGitHub(): Promise<void> {
    await open(GRIT_WORK_REPO_URL);
  }

  async function openUpstreamGitHub(): Promise<void> {
    await open(UPSTREAM_REPO_URL);
  }

  async function openDataDir(): Promise<void> {
    try {
      await invoke('open_data_dir');
    } catch (e) {
      console.error('Kunde inte öppna datamappen:', e);
    }
  }
</script>

<div class="page-shell about-editorial-shell" data-locale={currentLocale}>
  <div class="page-header page-axis-operation">
    <div class="page-title-group">
      <div class="page-title-badge">
        <Info size={18} strokeWidth={1.75} />
      </div>
      <div class="page-title-copy">
        <h2>{t('about.pageTitle')}</h2>
        <p>Version, lokal diagnostik och licens för GRIT Work.</p>
      </div>
    </div>
  </div>

  <div class="w-full about-minimal-shell page-axis-reading">
    <section class="page-card about-brand-card">
      <div class="about-brand-identity flex flex-col items-center">
        <div class="about-brand-mark">
          <img src="/icons/256x256.png" alt="GRIT Work" class="h-16 w-16 rounded-lg object-contain" style="image-rendering: pixelated;" />
        </div>

        <div class="about-brand-copy">
          <div class="about-brand-title-row flex items-center justify-center">
            <h1 class="about-brand-title">GRIT Work</h1>
            <span class="about-version-badge">v{appVersion}</span>
          </div>
          <p class="about-brand-description">
            Lokal skrivbordsklient för aktivitetsinsamling, arbetsöversikt och granskade tidrapporter till Spiris.
          </p>
        </div>
      </div>

      <div class="about-action-row mt-6">
        <button type="button" on:click={openGitHub} class="page-action-secondary min-h-10 px-4 py-2">
          <ExternalLink size={16} />
          <span class="leading-none">GitHub (GRIT Work)</span>
        </button>
        <button type="button" on:click={openDataDir} class="page-action-secondary min-h-10 px-4 py-2">
          <FolderOpen size={16} />
          <span class="leading-none">Öppna datamapp</span>
        </button>
        <button type="button" on:click={openUpstreamGitHub} class="page-action-secondary min-h-10 px-4 py-2">
          <span class="leading-none">Uppström: Work Review (MIT)</span>
        </button>
      </div>

      <div class="mt-5 space-y-2 text-center text-xs text-slate-500 dark:text-[#8b949e]" role="status" aria-live="polite">
        <p class="font-mono">
          Version: v{appVersion} · Plattform: {runtimePlatform}
        </p>
        <p>
          Utvecklingsförhandsvisning — automatisk uppdatering är avstängd i denna fork. Uppdatera manuellt via nya releaser eller källkod på GitHub.
        </p>
        <p>
          Byggd på öppen källkod från Work Review under MIT-licens.
        </p>
      </div>
    </section>

    <section class="page-card about-principles-card">
      <h2 class="about-principles-title page-section-title">Principer</h2>
      <div class="about-trust-grid mt-3 grid grid-cols-1 gap-3 md:grid-cols-3">
        <article class="about-trust-card rounded-xl border border-slate-200/80 bg-slate-50/50 p-4 dark:border-[#30363d] dark:bg-[#161b22]/40">
          <div class="flex items-center gap-2 text-slate-900 dark:text-[#e6edf3]">
            <Shield size={16} strokeWidth={1.75} class="shrink-0 text-indigo-600 dark:text-indigo-400" aria-hidden="true" />
            <h3 class="about-trust-title text-sm font-semibold">Lokal lagring först</h3>
          </div>
          <p class="about-trust-copy mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-[#8b949e]">Aktivitetsloggar och skärmbilder lagras lokalt eller i ditt verifierade arkiv innan något underlag analyseras.</p>
        </article>
        <article class="about-trust-card rounded-xl border border-slate-200/80 bg-slate-50/50 p-4 dark:border-[#30363d] dark:bg-[#161b22]/40">
          <div class="flex items-center gap-2 text-slate-900 dark:text-[#e6edf3]">
            <ListTree size={16} strokeWidth={1.75} class="shrink-0 text-indigo-600 dark:text-indigo-400" aria-hidden="true" />
            <h3 class="about-trust-title text-sm font-semibold">Spårbar tidslinje</h3>
          </div>
          <p class="about-trust-copy mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-[#8b949e]">Varje föreslagen tidrad kopplas till dokumenterat underlag och kan granskas mot tidslinjen.</p>
        </article>
        <article class="about-trust-card rounded-xl border border-slate-200/80 bg-slate-50/50 p-4 dark:border-[#30363d] dark:bg-[#161b22]/40">
          <div class="flex items-center gap-2 text-slate-900 dark:text-[#e6edf3]">
            <CircleCheck size={16} strokeWidth={1.75} class="shrink-0 text-indigo-600 dark:text-indigo-400" aria-hidden="true" />
            <h3 class="about-trust-title text-sm font-semibold">Manuellt godkännande</h3>
          </div>
          <p class="about-trust-copy mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-[#8b949e]">Inga tidrapporter skickas till Spiris utan att du själv granskat och bekräftat överföringen.</p>
        </article>
      </div>
    </section>
  </div>
</div>

<style>
  .about-trust-card {
    text-align: left;
  }

  .about-editorial-shell :global(.page-card),
  .about-trust-card,
  .about-brand-mark img {
    box-shadow: none;
  }
</style>
