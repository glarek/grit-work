<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import CaptureSettings from './CaptureSettings.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import {
    applyProviderChange,
    computeInvoiceableHours,
    createDefaultSettings,
    formatMinutesSwedish,
    formatSwedishHours,
    getStatusBadgeMeta,
    getYesterdayIsoDate,
    isKnownProjectId,
    isReportEditable,
    isReportReconcilable,
    normalizeGritReport,
    normalizeGritStatus,
    PROVIDER_LABELS,
    shiftIsoDateString,
    timeInputToUnixSeconds,
    unixSecondsToTimeInput,
    validateReportForSubmit,
    type GritArchiveResult,
    type GritProvider,
    type GritReport,
    type GritRow,
    type GritSettings,
    type GritStatus,
  } from './gritTypes.ts';

  type ActiveTab = 'report' | 'settings';
  type AuthMode = 'oauth' | 'static';

  let activeTab: ActiveTab = 'report';
  let status: GritStatus | null = null;
  let settingsForm: GritSettings = createDefaultSettings();
  let selectedDate = getYesterdayIsoDate();
  let report: GritReport | null = null;
  let dirtyDraft = false;

  let initialLoading = true;
  let reportLoading = false;
  let generating = false;
  let savingDraft = false;
  let submitting = false;
  let reconciling = false;
  let canceling = false;
  let syncingSpiris = false;
  let archiving = false;
  let savingSettings = false;
  let savingCredentials = false;

  let errorMessage: string | null = null;
  let noticeMessage: string | null = null;

  let laterDismissedDates = new Set<string>();
  let confirmSubmitOpen = false;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let dayTimer: ReturnType<typeof setInterval> | null = null;

  // Credentials state (memory only, never persisted to localStorage)
  let authMode: AuthMode = 'oauth';
  let applicationId = '';
  let secret = '';
  let staticToken = '';

  function formatError(err: unknown): string {
    if (typeof err === 'string' && err.trim()) return err;
    if (err instanceof Error && err.message.trim()) return err.message;
    return 'Ett oväntat fel uppstod.';
  }

  function clearFeedback(): void {
    errorMessage = null;
    noticeMessage = null;
  }

  function stopPolling(): void {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  function syncPollingState(): void {
    const state = report?.status;
    if (state === 'running' || state === 'uploading') {
      if (pollTimer === null) {
        pollTimer = setInterval(() => {
          void pollActiveReport();
        }, 3000);
      }
    } else {
      stopPolling();
    }
  }

  async function pollActiveReport(): Promise<void> {
    try {
      const [rawStatus, rawReport] = await Promise.all([
        invoke<unknown>('grit_status'),
        invoke<unknown>('grit_get_report', { date: selectedDate }),
      ]);
      const nextStatus = normalizeGritStatus(rawStatus);
      status = nextStatus;

      const nextReport =
        normalizeGritReport(rawReport) ??
        (nextStatus.report && nextStatus.report.date === selectedDate ? nextStatus.report : null);

      if (nextReport && !dirtyDraft) {
        report = nextReport;
      }

      if (nextReport?.status !== 'running') {
        generating = false;
      }
      syncPollingState();
    } catch (err) {
      console.warn('Pollningsfel i Grit Work:', err);
    }
  }

  async function loadStatusAndReport(dateOverride?: string): Promise<void> {
    clearFeedback();
    try {
      const rawStatus = await invoke<unknown>('grit_status');
      const nextStatus = normalizeGritStatus(rawStatus);
      status = nextStatus;
      settingsForm = { ...nextStatus.settings };

      const targetDate = dateOverride ?? (nextStatus.yesterday || getYesterdayIsoDate());
      selectedDate = targetDate;

      if (nextStatus.report && nextStatus.report.date === targetDate) {
        report = nextStatus.report;
      } else {
        const rawReport = await invoke<unknown>('grit_get_report', { date: targetDate });
        report = normalizeGritReport(rawReport);
      }
      dirtyDraft = false;
      generating = report?.status === 'running';
      syncPollingState();
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      initialLoading = false;
    }
  }

  async function selectDate(nextDate: string): Promise<void> {
    if (!nextDate) return;
    selectedDate = nextDate;
    reportLoading = true;
    clearFeedback();
    try {
      const rawReport = await invoke<unknown>('grit_get_report', { date: nextDate });
      report = normalizeGritReport(rawReport);
      dirtyDraft = false;
      generating = report?.status === 'running';
      syncPollingState();
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      reportLoading = false;
    }
  }

  async function handleGenerate(targetDate = selectedDate): Promise<void> {
    clearFeedback();
    selectedDate = targetDate;
    generating = true;
    activeTab = 'report';
    try {
      const rawReport = await invoke<unknown>('grit_generate', { date: targetDate });
      const nextReport = normalizeGritReport(rawReport);
      if (nextReport) {
        report = nextReport;
        dirtyDraft = false;
      }
      await refreshStatusOnly();
      if (report?.status === 'draft') noticeMessage = `Tidrapport för ${targetDate} genererad.`;
      else errorMessage = report?.error ?? 'Rapporten kunde inte genereras.';
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      generating = report?.status === 'running';
      syncPollingState();
    }
  }

  async function refreshStatusOnly(): Promise<void> {
    const rawStatus = await invoke<unknown>('grit_status');
    status = normalizeGritStatus(rawStatus);
  }

  async function handleCancel(): Promise<void> {
    clearFeedback();
    canceling = true;
    try {
      await invoke('grit_cancel');
      await selectDate(selectedDate);
      await refreshStatusOnly();
      noticeMessage = 'Genereringen avbröts.';
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      canceling = false;
      generating = false;
      syncPollingState();
    }
  }

  async function handleSaveDraft(): Promise<void> {
    if (!report || !isReportEditable(report.status)) return;
    clearFeedback();
    savingDraft = true;
    try {
      const rawSaved = await invoke<unknown>('grit_save_draft', { report });
      const saved = normalizeGritReport(rawSaved);
      if (saved) {
        report = saved;
      }
      dirtyDraft = false;
      await refreshStatusOnly();
      noticeMessage = 'Utkastet har sparats.';
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      savingDraft = false;
    }
  }

  async function handleConfirmSubmit(): Promise<void> {
    if (!report) return;
    clearFeedback();
    submitting = true;
    try {
      let currentReport = report;
      if (dirtyDraft) {
        const rawSaved = await invoke<unknown>('grit_save_draft', { report: currentReport });
        const saved = normalizeGritReport(rawSaved);
        if (saved) {
          currentReport = saved;
          report = saved;
          dirtyDraft = false;
        }
      }

      const rawSubmitted = await invoke<unknown>('grit_submit', {
        date: currentReport.date,
        revision: currentReport.revision,
      });
      const submitted = normalizeGritReport(rawSubmitted);
      if (submitted) {
        report = submitted;
      }
      confirmSubmitOpen = false;
      await refreshStatusOnly();
      if (report?.status === 'uploaded') noticeMessage = `Tidrapport för ${currentReport.date} skickades till Spiris.`;
      else errorMessage = report?.error ?? 'Överföringen är inte verifierad. Kontrollera status innan fortsatt arbete.';
    } catch (err) {
      confirmSubmitOpen = false;
      await selectDate(selectedDate);
      errorMessage = formatError(err);
    } finally {
      submitting = false;
      syncPollingState();
    }
  }

  async function handleReconcile(): Promise<void> {
    if (!report) return;
    clearFeedback();
    reconciling = true;
    try {
      const rawReconciled = await invoke<unknown>('grit_reconcile', { date: report.date });
      const reconciled = normalizeGritReport(rawReconciled);
      if (reconciled) {
        report = reconciled;
      }
      await refreshStatusOnly();
      noticeMessage = 'Status kontrollerad mot Spiris.';
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      reconciling = false;
      syncPollingState();
    }
  }

  async function handleDismissDate(date: string): Promise<void> {
    clearFeedback();
    try {
      await invoke('grit_dismiss', { date });
      await refreshStatusOnly();
      noticeMessage = `Påminnelse för ${date} hoppades över.`;
    } catch (err) {
      errorMessage = formatError(err);
    }
  }

  function handleLaterDate(date: string): void {
    const next = new Set(laterDismissedDates);
    next.add(date);
    laterDismissedDates = next;
  }

  async function handleSyncSpiris(): Promise<void> {
    clearFeedback();
    syncingSpiris = true;
    try {
      const rawStatus = await invoke<unknown>('grit_sync_spiris');
      status = normalizeGritStatus(rawStatus);
      noticeMessage = `Spiris synkroniserad (${status.projects.length} projekt, ${status.time_codes.length} tidskoder).`;
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      syncingSpiris = false;
    }
  }

  async function handleArchive(): Promise<void> {
    clearFeedback();
    archiving = true;
    try {
      const result = await invoke<GritArchiveResult>('grit_archive');
      noticeMessage = result?.message || `Arkiverade ${result?.archived ?? 0} poster.`;
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      archiving = false;
    }
  }

  async function handlePickArchiveDir(): Promise<void> {
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        defaultPath: settingsForm.archive_dir || status?.local_dir || undefined,
      });
      if (typeof picked === 'string' && picked.trim()) {
        settingsForm = { ...settingsForm, archive_dir: picked };
      }
    } catch (err) {
      errorMessage = formatError(err);
    }
  }

  function handleProviderSelect(nextProvider: GritProvider): void {
    settingsForm = applyProviderChange(settingsForm, nextProvider);
  }

  async function handleSaveSettings(): Promise<void> {
    clearFeedback();
    savingSettings = true;
    try {
      await invoke('grit_save_settings', { settings: settingsForm });
      await refreshStatusOnly();
      if (status) {
        settingsForm = { ...status.settings };
      }
      noticeMessage = 'Inställningarna har sparats.';
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      savingSettings = false;
    }
  }

  async function handleSaveCredentials(): Promise<void> {
    clearFeedback();
    savingCredentials = true;
    try {
      const payload =
        authMode === 'oauth'
          ? {
              applicationId: applicationId.trim(),
              secret: secret.trim(),
              staticToken: '',
            }
          : {
              applicationId: '',
              secret: '',
              staticToken: staticToken.trim(),
            };

      await invoke('grit_save_credentials', payload);
      applicationId = '';
      secret = '';
      staticToken = '';
      await refreshStatusOnly();
      noticeMessage = 'Spiris-uppgifterna har sparats säkert i backend.';
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      savingCredentials = false;
    }
  }

  function updateRow(index: number, patch: Partial<GritRow>): void {
    if (!report || !isReportEditable(report.status)) return;
    const nextRows = report.rows.map((row, idx) => (idx === index ? { ...row, ...patch } : row));
    report = { ...report, rows: nextRows };
    dirtyDraft = true;
  }

  function updateRowTime(index: number, field: 'start' | 'end', timeValue: string): void {
    if (!report || !isReportEditable(report.status)) return;
    const row = report.rows[index];
    if (!row) return;

    const parsedSeconds = timeInputToUnixSeconds(report.date, timeValue);
    if (parsedSeconds <= 0) return;

    const nextStart = field === 'start' ? parsedSeconds : row.start;
    const nextEnd = field === 'end' ? parsedSeconds : row.end;
    const computedMinutes =
      nextStart > 0 && nextEnd > nextStart ? Math.round((nextEnd - nextStart) / 60) : row.minutes;

    updateRow(index, {
      [field]: parsedSeconds,
      minutes: computedMinutes,
    });
  }

  function addManualRow(): void {
    if (!report || !isReportEditable(report.status) || !status) return;
    const startSec = timeInputToUnixSeconds(report.date, '09:00');
    const endSec = timeInputToUnixSeconds(report.date, '10:00');
    const newRow: GritRow = {
      id: `manual-${Date.now()}`,
      project_id: status.projects[0]?.id ?? 0,
      activity_id: status.settings.activity_id || status.activities[0]?.id || 0,
      article_id: status.settings.article_id || status.articles[0]?.id || 0,
      start: startSec,
      end: endSec,
      minutes: 60,
      description: '',
      evidence_ids: [],
      needs_review: true,
      remote_id: null,
    };
    report = { ...report, rows: [...report.rows, newRow] };
    dirtyDraft = true;
  }

  function removeRow(index: number): void {
    if (!report || !isReportEditable(report.status)) return;
    report = {
      ...report,
      rows: report.rows.filter((_, idx) => idx !== index),
    };
    dirtyDraft = true;
  }

  onMount(() => {
    void loadStatusAndReport();
    dayTimer = setInterval(async () => {
      try { status = normalizeGritStatus(await invoke('grit_status')); }
      catch { /* Keep the last known state while the backend is temporarily busy. */ }
    }, 60000);
  });

  onDestroy(() => {
    if (dayTimer !== null) clearInterval(dayTimer);
    stopPolling();
  });

  $: yesterdayDate = status?.yesterday || getYesterdayIsoDate();
  $: showYesterdayPrompt = Boolean(
    status &&
      status.pending_dates.includes(yesterdayDate) &&
      !laterDismissedDates.has(yesterdayDate) &&
      !(report?.date === yesterdayDate && report?.status === 'uploaded'),
  );
  $: editable = isReportEditable(report?.status);
  $: reconcilable = isReportReconcilable(report?.status);
  $: statusMeta = getStatusBadgeMeta(report?.status ?? '');
  $: submitValidation = validateReportForSubmit(report, status);
  $: providerInfo = PROVIDER_LABELS[settingsForm.provider];
  $: totalMinutes = report
    ? report.rows.reduce((sum, row) => sum + Math.max(0, row.minutes || 0), 0)
    : 0;
  $: billableEnabled = Boolean(status?.settings.billable);
  $: totalInvoiceableHours = computeInvoiceableHours(totalMinutes, billableEnabled);
</script>

<div class="grit-work mx-auto max-w-6xl space-y-5 px-4 py-6 sm:px-6">
  <!-- Header -->
  <header class="grit-header flex flex-wrap items-center justify-between gap-4 rounded-xl border border-slate-200/80 bg-white/90 p-5 shadow-sm backdrop-blur dark:border-[#30363d] dark:bg-[#161b22]/90">
    <div>
      <div class="flex items-center gap-2.5">
        <h1 class="text-xl font-semibold tracking-tight text-slate-900 dark:text-[#e6edf3]">
          Tidrapporter
        </h1>
        {#if status}
          <span
            class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium {status.credential_saved
              ? 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300'
              : 'bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300'}"
          >
            {status.credential_saved ? 'API-uppgifter sparade' : 'Spiris ej konfigurerad'}
          </span>
        {/if}
      </div>
      <p class="mt-1 text-sm text-slate-500 dark:text-[#8b949e]">
        Granska aktivitetsbaserade tidrapporter och skicka godkända rader till Spiris.
      </p>
    </div>

    <div class="flex flex-wrap items-center gap-2">
      <button
        type="button"
        on:click={handleSyncSpiris}
        disabled={syncingSpiris || initialLoading}
        class="rounded-xl border border-slate-200 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#c9d1d9] dark:hover:bg-[#30363d]"
      >
        {syncingSpiris ? 'Synkar Spiris...' : 'Synka Spiris'}
      </button>

      <button
        type="button"
        on:click={handleArchive}
        disabled={archiving || initialLoading}
        class="rounded-xl border border-slate-200 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#c9d1d9] dark:hover:bg-[#30363d]"
      >
        {archiving ? 'Arkiverar...' : 'Arkivera'}
      </button>

      <div class="ml-1 inline-flex rounded-xl border border-slate-200 bg-slate-100/80 p-1 dark:border-[#30363d] dark:bg-[#0d1117]">
        <button
          type="button"
          on:click={() => (activeTab = 'report')}
          class="rounded-lg px-3 py-1.5 text-xs font-semibold transition {activeTab === 'report'
            ? 'bg-white text-slate-900 shadow-sm dark:bg-[#21262d] dark:text-[#e6edf3]'
            : 'text-slate-600 hover:text-slate-900 dark:text-[#8b949e] dark:hover:text-[#e6edf3]'}"
        >
          Tidrapport
        </button>
        <button
          type="button"
          on:click={() => (activeTab = 'settings')}
          class="rounded-lg px-3 py-1.5 text-xs font-semibold transition {activeTab === 'settings'
            ? 'bg-white text-slate-900 shadow-sm dark:bg-[#21262d] dark:text-[#e6edf3]'
            : 'text-slate-600 hover:text-slate-900 dark:text-[#8b949e] dark:hover:text-[#e6edf3]'}"
        >
          Inställningar
        </button>
      </div>
    </div>
  </header>

  <!-- Feedback banners -->
  {#if errorMessage || status?.last_error}
    <div class="rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-800 dark:border-rose-800/60 dark:bg-rose-950/40 dark:text-rose-200">
      <div class="font-semibold">Fel</div>
      <p class="mt-0.5 whitespace-pre-wrap">{errorMessage ?? status?.last_error}</p>
    </div>
  {/if}

  {#if noticeMessage}
    <div class="rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800 dark:border-emerald-800/60 dark:bg-emerald-950/40 dark:text-emerald-200">
      {noticeMessage}
    </div>
  {/if}

  {#if initialLoading}
    <div class="rounded-xl border border-slate-200/80 bg-white/90 p-10 text-center text-sm text-slate-500 dark:border-[#30363d] dark:bg-[#161b22]/90 dark:text-[#8b949e]">
      Laddar Grit Work...
    </div>
  {:else if activeTab === 'report'}
    <!-- Yesterday prompt -->
    {#if showYesterdayPrompt}
      <section class="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-indigo-200 bg-indigo-50/90 p-4 dark:border-indigo-800/60 dark:bg-indigo-950/30">
        <div>
          <h2 class="text-sm font-semibold text-indigo-950 dark:text-indigo-200">
            Gårdagens tidrapport ({yesterdayDate}) saknas
          </h2>
          <p class="mt-0.5 text-xs text-indigo-800/80 dark:text-indigo-300/80">
            Vill du generera ett utkast från gårdagens insamlade aktivitet?
          </p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <button
            type="button"
            on:click={() => handleGenerate(yesterdayDate)}
            disabled={generating}
            class="rounded-xl bg-indigo-600 px-3.5 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
          >
            Skapa rapport
          </button>
          <button
            type="button"
            on:click={() => handleLaterDate(yesterdayDate)}
            class="rounded-xl border border-indigo-200 bg-white px-3.5 py-2 text-xs font-semibold text-indigo-800 transition hover:bg-indigo-100/60 dark:border-indigo-700/60 dark:bg-[#161b22] dark:text-indigo-200"
          >
            Senare
          </button>
          <button
            type="button"
            on:click={() => handleDismissDate(yesterdayDate)}
            class="rounded-xl px-3 py-2 text-xs font-semibold text-indigo-700 transition hover:bg-indigo-100/80 dark:text-indigo-300 dark:hover:bg-indigo-900/40"
          >
            Hoppa över
          </button>
        </div>
      </section>
    {/if}

    <!-- Date selector & pending dates -->
    <section class="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-slate-200/80 bg-white/90 p-4 shadow-sm dark:border-[#30363d] dark:bg-[#161b22]/90">
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          on:click={() => selectDate(shiftIsoDateString(selectedDate, -1))}
          class="rounded-lg border border-slate-200 px-2.5 py-1.5 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-[#30363d] dark:text-[#c9d1d9] dark:hover:bg-[#21262d]"
          aria-label="Föregående dag"
        >
          ←
        </button>
        <input
          type="date"
          bind:value={selectedDate}
          on:change={() => selectDate(selectedDate)}
          class="rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm font-medium text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
        />
        <button
          type="button"
          on:click={() => selectDate(shiftIsoDateString(selectedDate, 1))}
          class="rounded-lg border border-slate-200 px-2.5 py-1.5 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-[#30363d] dark:text-[#c9d1d9] dark:hover:bg-[#21262d]"
          aria-label="Nästa dag"
        >
          →
        </button>

        {#if status && status.pending_dates.length > 0}
          <div class="ml-2 flex flex-wrap items-center gap-1.5">
            <span class="text-xs text-slate-500 dark:text-[#8b949e]">Väntande:</span>
            {#each status.pending_dates as pendingDate}
              <button
                type="button"
                on:click={() => selectDate(pendingDate)}
                class="rounded-full px-2.5 py-1 text-xs font-medium transition {selectedDate === pendingDate
                  ? 'bg-indigo-600 text-white'
                  : 'bg-slate-100 text-slate-700 hover:bg-slate-200 dark:bg-[#21262d] dark:text-[#c9d1d9]'}"
              >
                {pendingDate}
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="flex flex-wrap items-center gap-2">
        {#if report?.status === 'running' || generating}
          <button
            type="button"
            on:click={handleCancel}
            disabled={canceling}
            class="rounded-xl border border-rose-200 bg-rose-50 px-3.5 py-2 text-xs font-semibold text-rose-700 transition hover:bg-rose-100 disabled:opacity-50 dark:border-rose-800/60 dark:bg-rose-950/40 dark:text-rose-300"
          >
            {canceling ? 'Avbryter...' : 'Avbryt generering'}
          </button>
        {:else if !report || editable}
          <button
            type="button"
            on:click={() => handleGenerate(selectedDate)}
            disabled={generating || reportLoading}
            class="rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
          >
            {report ? 'Generera om utkast' : 'Generera tidrapport'}
          </button>
        {/if}
      </div>
    </section>

    <!-- Report body -->
    {#if reportLoading}
      <div class="rounded-xl border border-slate-200/80 bg-white/90 p-10 text-center text-sm text-slate-500 dark:border-[#30363d] dark:bg-[#161b22]/90 dark:text-[#8b949e]">
        Hämtar tidrapport för {selectedDate}...
      </div>
    {:else if !report}
      <div class="rounded-xl border border-dashed border-slate-300 bg-white/70 p-10 text-center dark:border-[#30363d] dark:bg-[#161b22]/60">
        <h2 class="text-base font-semibold text-slate-900 dark:text-[#e6edf3]">
          Ingen tidrapport för {selectedDate}
        </h2>
        <p class="mt-1 text-sm text-slate-500 dark:text-[#8b949e]">
          Klicka på "Generera tidrapport" för att låta vald AI-CLI sammanställa dagens arbete till ett granskningsbart utkast.
        </p>
      </div>
    {:else}
      <!-- Report status & totals card -->
      <section class="space-y-4 rounded-xl border border-slate-200/80 bg-white/90 p-5 shadow-sm dark:border-[#30363d] dark:bg-[#161b22]/90">
        <div class="flex flex-wrap items-start justify-between gap-4">
          <div class="space-y-1">
            <div class="flex flex-wrap items-center gap-2.5">
              <span class="text-base font-semibold text-slate-900 dark:text-[#e6edf3]">
                Rapport {report.date}
              </span>
              <span
                class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold {statusMeta.tone === 'success'
                  ? 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300'
                  : statusMeta.tone === 'warning'
                    ? 'bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300'
                    : statusMeta.tone === 'danger'
                      ? 'bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-300'
                      : statusMeta.tone === 'info'
                        ? 'bg-sky-100 text-sky-800 dark:bg-sky-900/40 dark:text-sky-300'
                        : 'bg-slate-100 text-slate-700 dark:bg-[#21262d] dark:text-[#c9d1d9]'}"
              >
                {statusMeta.label}
              </span>
              {#if dirtyDraft}
                <span class="rounded-full bg-amber-100 px-2 py-0.5 text-[11px] font-medium text-amber-800 dark:bg-amber-900/40 dark:text-amber-300">
                  Osparade ändringar
                </span>
              {/if}
            </div>
            <p class="text-xs text-slate-500 dark:text-[#8b949e]">
              {statusMeta.description}
            </p>
          </div>

          <div class="flex flex-wrap items-center gap-4 rounded-xl border border-slate-200/80 bg-slate-50/80 px-4 py-2.5 dark:border-[#30363d] dark:bg-[#0d1117]">
            <div>
              <div class="text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-[#8b949e]">
                Total tid
              </div>
              <div class="grit-metric text-sm font-semibold text-slate-900 dark:text-[#e6edf3]">
                {formatMinutesSwedish(totalMinutes)} <span class="text-xs font-normal text-slate-500">· {report.rows.length} {report.rows.length === 1 ? 'rad' : 'rader'}</span>
              </div>
            </div>
            <div class="h-7 w-px bg-slate-200 dark:bg-[#30363d]"></div>
            <div>
              <div class="text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-[#8b949e]">
                Fakturerbar tid (Spiris)
              </div>
              <div class="grit-metric text-sm font-semibold text-slate-900 dark:text-[#e6edf3]">
                {formatSwedishHours(totalInvoiceableHours)}
                <span class="ml-1 text-xs font-normal text-slate-500 dark:text-[#8b949e]">
                  ({billableEnabled ? 'Debiterbar tid: På' : 'Debiterbar tid: Av → 0 h'})
                </span>
              </div>
            </div>
          </div>
        </div>

        {#if report.summary}
          <div class="grit-summary rounded-xl bg-slate-50 p-3 text-sm text-slate-700 dark:bg-[#0d1117] dark:text-[#c9d1d9]">
            <span class="font-semibold text-slate-900 dark:text-[#e6edf3]">Sammanfattning: </span>
            <span>{report.summary}</span>
          </div>
        {/if}

        {#if report.error}
          <div class="rounded-xl border border-rose-200 bg-rose-50 p-3 text-sm text-rose-800 dark:border-rose-800/60 dark:bg-rose-950/40 dark:text-rose-200">
            {report.error}
          </div>
        {/if}

        {#if reconcilable}
          <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-amber-200 bg-amber-50 p-4 dark:border-amber-800/60 dark:bg-amber-950/30">
            <div class="text-sm text-amber-900 dark:text-amber-200">
              <div class="font-semibold">Återställning krävs — skicka inte på nytt i blindo</div>
              <p class="mt-0.5 text-xs">
                Kontrollera vilka rader som redan registrerats i Spiris innan ytterligare åtgärder.
              </p>
            </div>
            <button
              type="button"
              on:click={handleReconcile}
              disabled={reconciling}
              class="rounded-xl bg-amber-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-amber-700 disabled:opacity-50"
            >
              {reconciling ? 'Kontrollerar...' : 'Kontrollera i Spiris'}
            </button>
          </div>
        {/if}

        <!-- Rows table / cards -->
        <div class="space-y-3">
          {#if report.rows.length === 0}
            <div class="rounded-xl border border-dashed border-slate-200 p-6 text-center text-sm text-slate-500 dark:border-[#30363d] dark:text-[#8b949e]">
              Inga tidrader i denna rapport.
            </div>
          {:else}
            {#each report.rows as row, index (row.id)}
              {@const knownProject = isKnownProjectId(row.project_id, status?.projects ?? [])}
              {@const rowInvoiceable = computeInvoiceableHours(row.minutes, billableEnabled)}
              <div class="grit-row rounded-xl border p-4 transition {row.needs_review
                ? 'border-amber-300 bg-amber-50/40 dark:border-amber-700/60 dark:bg-amber-950/15'
                : 'border-slate-200 bg-white dark:border-[#30363d] dark:bg-[#0d1117]/60'}">
                <div class="grid grid-cols-1 gap-3 md:grid-cols-12">
                  <!-- Project -->
                  <div class="md:col-span-4">
                    <label class="block text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8b949e]">
                      Projekt
                      <select
                        value={row.project_id}
                        disabled={!editable}
                        on:change={(e) =>
                          updateRow(index, { project_id: Number(e.currentTarget.value) || 0 })}
                        class="mt-1 w-full rounded-lg border px-2.5 py-1.5 text-sm dark:bg-[#161b22] dark:text-[#e6edf3] {!knownProject
                          ? 'border-rose-400 bg-rose-50 text-rose-900 dark:border-rose-700 dark:bg-rose-950/30'
                          : 'border-slate-200 bg-white text-slate-900 dark:border-[#30363d]'}"
                      >
                        {#if !knownProject}
                          <option value={row.project_id}>
                            {row.project_id > 0
                              ? `Okänt projekt-ID (${row.project_id}) — välj giltigt`
                              : 'Välj projekt...'}
                          </option>
                        {/if}
                        {#each status?.projects ?? [] as project (project.id)}
                          <option value={project.id}>
                            {project.number ? `${project.number} · ` : ''}{project.name}{project.active ? '' : ' (inaktiv)'}
                          </option>
                        {/each}
                      </select>
                    </label>
                  </div>

                  <!-- Activity -->
                  <div class="md:col-span-3">
                    <label class="block text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8b949e]">
                      Aktivitet
                      <select
                        value={row.activity_id}
                        disabled={!editable}
                        on:change={(e) =>
                          updateRow(index, { activity_id: Number(e.currentTarget.value) || 0 })}
                        class="mt-1 w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#161b22] dark:text-[#e6edf3]"
                      >
                        <option value={0}>Ingen / standard</option>
                        {#each status?.activities ?? [] as activity (activity.id)}
                          <option value={activity.id}>{activity.name}</option>
                        {/each}
                      </select>
                    </label>
                  </div>

                  <!-- Article -->
                  <div class="md:col-span-3">
                    <label class="block text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8b949e]">
                      Artikel
                      <select
                        value={row.article_id}
                        disabled={!editable}
                        on:change={(e) =>
                          updateRow(index, { article_id: Number(e.currentTarget.value) || 0 })}
                        class="mt-1 w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#161b22] dark:text-[#e6edf3]"
                      >
                        <option value={0}>Ingen / standard</option>
                        {#each status?.articles ?? [] as article (article.id)}
                          <option value={article.id}>{article.name}</option>
                        {/each}
                      </select>
                    </label>
                  </div>

                  <!-- Minutes -->
                  <div class="md:col-span-2">
                    <label class="block text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8b949e]">
                      Minuter
                      <input
                        type="number"
                        min="0"
                        step="5"
                        value={row.minutes}
                        disabled={!editable}
                        on:input={(e) =>
                          updateRow(index, {
                            minutes: Math.max(0, Math.trunc(Number(e.currentTarget.value) || 0)),
                          })}
                        class="mt-1 w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#161b22] dark:text-[#e6edf3]"
                      />
                    </label>
                  </div>

                  <!-- Start / End -->
                  <div class="md:col-span-2">
                    <label class="block text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8b949e]">
                      Start
                      <input
                        type="time"
                        value={unixSecondsToTimeInput(row.start)}
                        disabled={!editable}
                        on:change={(e) => updateRowTime(index, 'start', e.currentTarget.value)}
                        class="mt-1 w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#161b22] dark:text-[#e6edf3]"
                      />
                    </label>
                  </div>

                  <div class="md:col-span-2">
                    <label class="block text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8b949e]">
                      Slut
                      <input
                        type="time"
                        value={unixSecondsToTimeInput(row.end)}
                        disabled={!editable}
                        on:change={(e) => updateRowTime(index, 'end', e.currentTarget.value)}
                        class="mt-1 w-full rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#161b22] dark:text-[#e6edf3]"
                      />
                    </label>
                  </div>

                  <!-- Description -->
                  <div class="md:col-span-8">
                    <label class="block text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8b949e]">
                      Beskrivning
                      <input
                        type="text"
                        value={row.description}
                        disabled={!editable}
                        placeholder="Vad gjordes under detta arbetspass?"
                        on:input={(e) => updateRow(index, { description: e.currentTarget.value })}
                        class="mt-1 w-full rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#161b22] dark:text-[#e6edf3]"
                      />
                    </label>
                  </div>
                </div>

                <div class="mt-3 flex flex-wrap items-center justify-between gap-3 border-t border-slate-200/70 pt-2.5 text-xs dark:border-[#30363d]">
                  <div class="flex flex-wrap items-center gap-3">
                    <label class="inline-flex cursor-pointer items-center gap-2 font-medium text-slate-700 dark:text-[#c9d1d9]">
                      <input
                        type="checkbox"
                        checked={row.needs_review}
                        disabled={!editable}
                        on:change={(e) =>
                          updateRow(index, { needs_review: e.currentTarget.checked })}
                        class="h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
                      />
                      Kräver granskning
                    </label>

                    <span class="text-slate-500 dark:text-[#8b949e]">
                      Fakturerbart: {formatSwedishHours(rowInvoiceable)}
                    </span>

                    {#if row.evidence_ids.length > 0}
                      <span class="text-slate-500 dark:text-[#8b949e]">
                        Underlag: {row.evidence_ids.map((id) => `#${id}`).join(', ')}
                      </span>
                    {/if}

                    {#if row.remote_id !== null}
                      <span class="rounded bg-emerald-100 px-2 py-0.5 font-medium text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300">
                        Spiris-ID #{row.remote_id}
                      </span>
                    {/if}
                  </div>

                  {#if editable}
                    <button
                      type="button"
                      on:click={() => removeRow(index)}
                      class="font-medium text-rose-600 hover:text-rose-700 dark:text-rose-400"
                    >
                      Ta bort rad
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          {/if}
        </div>

        {#if editable}
          <div class="flex justify-start">
            <button
              type="button"
              on:click={addManualRow}
              class="rounded-xl border border-dashed border-slate-300 px-3.5 py-2 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 dark:border-[#30363d] dark:text-[#c9d1d9] dark:hover:bg-[#21262d]"
            >
              + Lägg till rad
            </button>
          </div>
        {/if}

        <!-- Blockers & Actions -->
        {#if submitValidation.blockers.length > 0 && report.status !== 'uploaded'}
          <div class="rounded-xl border border-slate-200 bg-slate-50 p-3.5 text-xs text-slate-700 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#c9d1d9]">
            <div class="font-semibold text-slate-900 dark:text-[#e6edf3]">
              Krav innan tidrapporten kan skickas till Spiris:
            </div>
            <ul class="mt-1.5 list-disc space-y-0.5 pl-4">
              {#each submitValidation.blockers as blocker}
                <li>{blocker}</li>
              {/each}
            </ul>
          </div>
        {/if}

        <div class="flex flex-wrap items-center justify-end gap-3 border-t border-slate-200/80 pt-4 dark:border-[#30363d]">
          {#if editable}
            <button
              type="button"
              on:click={handleSaveDraft}
              disabled={savingDraft}
              class="rounded-xl border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-800 transition hover:bg-slate-50 disabled:opacity-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#e6edf3]"
            >
              {savingDraft ? 'Sparar utkast...' : 'Spara utkast'}
            </button>

            <button
              type="button"
              on:click={() => (confirmSubmitOpen = true)}
              disabled={!submitValidation.canSubmit || submitting || savingDraft}
              class="rounded-xl bg-emerald-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              Godkänn & skicka till Spiris
            </button>
          {/if}
        </div>
      </section>
    {/if}
  {:else}
    <!-- Settings tab -->
    {#if status}
      <CaptureSettings settings={status.capture} monitors={status.monitors} on:saved={refreshStatusOnly} />
    {/if}
    <div class="grid grid-cols-1 gap-5 lg:grid-cols-2">
      <!-- AI & Spiris defaults -->
      <section class="space-y-4 rounded-xl border border-slate-200/80 bg-white/90 p-5 shadow-sm dark:border-[#30363d] dark:bg-[#161b22]/90">
        <div>
          <h2 class="text-base font-semibold text-slate-900 dark:text-[#e6edf3]">
            Generering & Spiris-standarder
          </h2>
          <p class="mt-0.5 text-xs text-slate-500 dark:text-[#8b949e]">
            Välj vilken lokal AI-CLI som skapar tidrapportutkast och vilka standardvärden som skickas till Spiris.
          </p>
        </div>

        <div class="space-y-3">
          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              AI-leverantör (lokal CLI)
              <select
                value={settingsForm.provider}
                on:change={(e) => {
                  const val = e.currentTarget.value;
                  if (val === 'codex' || val === 'grok' || val === 'agy') {
                    handleProviderSelect(val);
                  }
                }}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value="codex">Codex CLI (OpenAI)</option>
                <option value="grok">Grok CLI (xAI)</option>
                <option value="agy">Antigravity CLI (Google Gemini)</option>
              </select>
            </label>
            <p class="mt-1.5 rounded-lg bg-slate-50 p-2.5 text-xs text-slate-600 dark:bg-[#0d1117] dark:text-[#8b949e]">
              {providerInfo.description}
            </p>
          </div>

          <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Modell
              <input
                type="text"
                bind:value={settingsForm.model}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              />
            </label>

            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Timeout (sekunder)
              <input
                type="number"
                min="30"
                step="30"
                bind:value={settingsForm.timeout_seconds}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              />
            </label>
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Arkivmapp (valfri)
              <div class="mt-1 flex gap-2">
                <input
                  type="text"
                  bind:value={settingsForm.archive_dir}
                  placeholder={status?.local_dir || 'Lokal arbetsmapp'}
                  class="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
                />
                <button
                  type="button"
                  on:click={handlePickArchiveDir}
                  class="shrink-0 rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-100 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#c9d1d9]"
                >
                  Välj mapp
                </button>
              </div>
            </label>
          </div>

          <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Spiris-användare (krävs)
              <select
                value={settingsForm.user_id}
                on:change={(e) =>
                  (settingsForm = { ...settingsForm, user_id: Number(e.currentTarget.value) || 0 })}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value={0}>Välj användare...</option>
                {#each status?.users ?? [] as user (user.id)}
                  <option value={user.id}>{user.name}</option>
                {/each}
              </select>
            </label>

            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Tidskod (krävs)
              <select
                value={settingsForm.time_code_id}
                on:change={(e) =>
                  (settingsForm = {
                    ...settingsForm,
                    time_code_id: Number(e.currentTarget.value) || 0,
                  })}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value={0}>Välj tidskod...</option>
                {#each status?.time_codes ?? [] as tc (tc.id)}
                  <option value={tc.id}>{tc.name}</option>
                {/each}
              </select>
            </label>

            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Standardaktivitet
              <select
                value={settingsForm.activity_id}
                on:change={(e) =>
                  (settingsForm = {
                    ...settingsForm,
                    activity_id: Number(e.currentTarget.value) || 0,
                  })}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value={0}>Ingen standard</option>
                {#each status?.activities ?? [] as act (act.id)}
                  <option value={act.id}>{act.name}</option>
                {/each}
              </select>
            </label>

            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Standardartikel
              <select
                value={settingsForm.article_id}
                on:change={(e) =>
                  (settingsForm = {
                    ...settingsForm,
                    article_id: Number(e.currentTarget.value) || 0,
                  })}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              >
                <option value={0}>Ingen standard</option>
                {#each status?.articles ?? [] as art (art.id)}
                  <option value={art.id}>{art.name}</option>
                {/each}
              </select>
            </label>
          </div>

          <label class="flex cursor-pointer items-start gap-2.5 rounded-xl border border-slate-200/80 bg-slate-50/70 p-3 dark:border-[#30363d] dark:bg-[#0d1117]">
            <input
              type="checkbox"
              bind:checked={settingsForm.billable}
              class="mt-0.5 h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
            />
            <div>
              <div class="text-xs font-semibold text-slate-900 dark:text-[#e6edf3]">
                Debiterbar tid
              </div>
              <p class="mt-0.5 text-xs text-slate-500 dark:text-[#8b949e]">
                Markera om den rapporterade tiden ska faktureras kunden.
              </p>
            </div>
          </label>
        </div>

        <div class="flex justify-end pt-2">
          <button
            type="button"
            on:click={handleSaveSettings}
            disabled={savingSettings}
            class="rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
          >
            {savingSettings ? 'Sparar...' : 'Spara inställningar'}
          </button>
        </div>
      </section>

      <!-- Spiris Credentials -->
      <section class="space-y-4 rounded-xl border border-slate-200/80 bg-white/90 p-5 shadow-sm dark:border-[#30363d] dark:bg-[#161b22]/90">
        <div>
          <h2 class="text-base font-semibold text-slate-900 dark:text-[#e6edf3]">
            Anslutning till Spiris
          </h2>
          <p class="mt-0.5 text-xs text-slate-500 dark:text-[#8b949e]">
            Dina uppgifter sparas skyddat i datorns nyckellager.
          </p>
        </div>

        <div class="inline-flex rounded-xl border border-slate-200 bg-slate-100/80 p-1 dark:border-[#30363d] dark:bg-[#0d1117]">
          <button
            type="button"
            on:click={() => (authMode = 'oauth')}
            class="rounded-lg px-3 py-1.5 text-xs font-semibold transition {authMode === 'oauth'
              ? 'bg-white text-slate-900 shadow-sm dark:bg-[#21262d] dark:text-[#e6edf3]'
              : 'text-slate-600 dark:text-[#8b949e]'}"
          >
            Application ID + Secret
          </button>
          <button
            type="button"
            on:click={() => (authMode = 'static')}
            class="rounded-lg px-3 py-1.5 text-xs font-semibold transition {authMode === 'static'
              ? 'bg-white text-slate-900 shadow-sm dark:bg-[#21262d] dark:text-[#e6edf3]'
              : 'text-slate-600 dark:text-[#8b949e]'}"
          >
            Statisk token
          </button>
        </div>

        {#if authMode === 'oauth'}
          <div class="space-y-3">
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Application ID
              <input
                type="password"
                autocomplete="off"
                bind:value={applicationId}
                placeholder={status?.credential_saved ? '•••••••• (sparad)' : 'Ange Application ID'}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              />
            </label>
            <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
              Client Secret
              <input
                type="password"
                autocomplete="off"
                bind:value={secret}
                placeholder={status?.credential_saved ? '•••••••• (sparad)' : 'Ange Client Secret'}
                class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
              />
            </label>
          </div>
        {:else}
          <label class="block text-xs font-semibold text-slate-700 dark:text-[#c9d1d9]">
            Statisk Bearer-token
            <input
              type="password"
              autocomplete="off"
              bind:value={staticToken}
              placeholder={status?.credential_saved ? '•••••••• (sparad)' : 'Ange statisk token'}
              class="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 dark:border-[#30363d] dark:bg-[#0d1117] dark:text-[#e6edf3]"
            />
          </label>
        {/if}

        <div class="flex items-center justify-between pt-2">
          <span class="text-xs text-slate-500 dark:text-[#8b949e]">
            {status?.credential_saved ? 'Uppgifterna är sparade i datorns nyckellager.' : 'Inga uppgifter sparade ännu.'}
          </span>
          <button
            type="button"
            on:click={handleSaveCredentials}
            disabled={
              savingCredentials ||
              (authMode === 'oauth' ? !applicationId.trim() || !secret.trim() : !staticToken.trim())
            }
            class="rounded-xl bg-slate-900 px-4 py-2 text-xs font-semibold text-white transition hover:bg-slate-800 disabled:opacity-50 dark:bg-[#238636] dark:hover:bg-[#2ea043]"
          >
            {savingCredentials ? 'Sparar...' : 'Spara uppgifter'}
          </button>
        </div>
      </section>
    </div>
  {/if}
</div>

<!-- Explicit confirmation modal before submit -->
{#if confirmSubmitOpen && report}
  <div
    class="fixed inset-0 z-[120] flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-sm"
    role="dialog"
    aria-modal="true"
    aria-labelledby="grit-confirm-title"
  >
    <div class="w-full max-w-md space-y-4 rounded-xl border border-slate-200 bg-white p-6 shadow-2xl dark:border-[#30363d] dark:bg-[#161b22]">
      <div>
        <h3 id="grit-confirm-title" class="text-base font-semibold text-slate-900 dark:text-[#e6edf3]">
          Bekräfta överföring till Spiris
        </h3>
        <p class="mt-1 text-sm text-slate-600 dark:text-[#8b949e]">
          Efter att tidrapporten skickats låses den mot vidare redigering.
        </p>
      </div>

      <dl class="space-y-2 rounded-xl bg-slate-50 p-3.5 text-sm dark:bg-[#0d1117]">
        <div class="flex justify-between">
          <dt class="text-slate-500 dark:text-[#8b949e]">Datum</dt>
          <dd class="font-semibold text-slate-900 dark:text-[#e6edf3]">{report.date}</dd>
        </div>
        <div class="flex justify-between">
          <dt class="text-slate-500 dark:text-[#8b949e]">Antal rader</dt>
          <dd class="font-semibold text-slate-900 dark:text-[#e6edf3]">{report.rows.length}</dd>
        </div>
        <div class="flex justify-between">
          <dt class="text-slate-500 dark:text-[#8b949e]">Total tid</dt>
          <dd class="font-semibold text-slate-900 dark:text-[#e6edf3]">{formatMinutesSwedish(totalMinutes)}</dd>
        </div>
        <div class="flex justify-between">
          <dt class="text-slate-500 dark:text-[#8b949e]">Fakturerbar tid</dt>
          <dd class="font-semibold text-slate-900 dark:text-[#e6edf3]">
            {formatSwedishHours(totalInvoiceableHours)}
          </dd>
        </div>
      </dl>

      <div class="flex justify-end gap-2.5 pt-1">
        <button
          type="button"
          on:click={() => (confirmSubmitOpen = false)}
          disabled={submitting}
          class="rounded-xl border border-slate-200 bg-white px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#c9d1d9]"
        >
          Avbryt
        </button>
        <button
          type="button"
          on:click={handleConfirmSubmit}
          disabled={submitting}
          class="rounded-xl bg-emerald-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-emerald-700 disabled:opacity-50"
        >
          {submitting ? 'Skickar...' : 'Bekräfta & skicka'}
        </button>
      </div>
    </div>
  </div>
{/if}
