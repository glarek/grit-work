<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-shell';
  import { getVersion } from '@tauri-apps/api/app';
  import { locale, t } from '$lib/i18n/index.ts';

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
        <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="8.5" stroke-width="1.8" />
          <path stroke-linecap="round" stroke-width="1.8" d="M12 10.5v5" />
          <circle cx="12" cy="7.5" r=".8" fill="currentColor" stroke="none" />
        </svg>
      </div>
      <div class="page-title-copy">
        <h2>{t('about.pageTitle')}</h2>
        <p>Versionsinformation, lokal diagnostik och licensuppgifter för Grit Work.</p>
      </div>
    </div>
  </div>

  <div class="w-full about-minimal-shell page-axis-reading">
    <section class="page-card about-brand-card">
      <div class="about-brand-identity flex flex-col items-center">
        <div class="about-brand-mark">
          <img src="/icons/256x256.png" alt="Grit Work" class="h-16 w-16 rounded-lg object-cover" />
        </div>

        <div class="about-brand-copy">
          <div class="about-brand-title-row flex items-center justify-center">
            <h1 class="about-brand-title">Grit Work</h1>
            <span class="about-version-badge">v{appVersion}</span>
          </div>
          <p class="about-brand-description">
            Lokal skrivbordsklient för aktivitetsinsamling, tidsanalys och granskade tidrapporter till Spiris.
          </p>
        </div>
      </div>

      <div class="about-action-row mt-6">
        <button type="button" on:click={openGitHub} class="page-action-secondary min-h-10 px-4 py-2">
          <svg class="w-4 h-4 shrink-0" fill="currentColor" viewBox="0 0 24 24"><path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/></svg>
          <span class="leading-none">GitHub (Grit Work)</span>
        </button>
        <button type="button" on:click={openDataDir} class="page-action-secondary min-h-10 px-4 py-2">
          <svg class="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/></svg>
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
      <h2 class="about-principles-title page-section-title text-center">Principer</h2>
      <div class="about-trust-grid">
        <article class="about-trust-card">
          <span class="about-trust-kicker">01</span>
          <h3 class="about-trust-title">Lokal lagring först</h3>
          <p class="about-trust-copy">Aktivitetsloggar och skärmbilder lagras lokalt eller i ditt verifierade arkiv innan något underlag analyseras.</p>
        </article>
        <article class="about-trust-card">
          <span class="about-trust-kicker">02</span>
          <h3 class="about-trust-title">Spårbar tidslinje</h3>
          <p class="about-trust-copy">Varje föreslagen tidrad kopplas till dokumenterat underlag och kan granskas mot tidslinjen.</p>
        </article>
        <article class="about-trust-card">
          <span class="about-trust-kicker">03</span>
          <h3 class="about-trust-title">Manuellt godkännande</h3>
          <p class="about-trust-copy">Inga tidrapporter skickas till Spiris utan att du själv granskat och bekräftat överföringen.</p>
        </article>
      </div>
    </section>

    <section class="page-card about-tech-stack">
      <span class="about-tech-item">Tauri 2</span>
      <span class="about-tech-item">Svelte</span>
      <span class="about-tech-item">Rust</span>
      <span class="about-tech-item">SQLite</span>
    </section>
  </div>
</div>
