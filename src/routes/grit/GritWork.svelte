<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import ReportingDialog from './ReportingDialog.svelte';
  import ReportRowEditor from './ReportRowEditor.svelte';
  import LocalizedDatePicker from '../../lib/components/LocalizedDatePicker.svelte';
  import GritIcon from './GritIcon.svelte';
  import { get } from 'svelte/store';
  import WorkflowBar from './WorkflowBar.svelte';
  import { workDate, selectWorkDate, notifyGritChanged } from './workflow.ts';
  import { invoke } from '@tauri-apps/api/core';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import {
    computeInvoiceableHours,
    computeProjectRounding,
    computeUnassignedGaps,
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
    type GritReport,
    type GritRow,
    type GritSettings,
    type GritStatus,
    type UnassignedGap,
  } from './gritTypes.ts';

  let status: GritStatus | null = null;
  let settingsForm: GritSettings = createDefaultSettings();
  let selectedDate = get(workDate);
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

  let errorMessage: string | null = null;
  let noticeMessage: string | null = null;

  let laterDismissedDates = new Set<string>();
  let confirmSubmitOpen = false;
  let editingRowIndex: number | null = null;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let dayTimer: ReturnType<typeof setInterval> | null = null;

  // Credentials state (memory only, never persisted to localStorage)

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
    const requestedDate = selectedDate;
    try {
      const [rawStatus, rawReport] = await Promise.all([
        invoke<unknown>('grit_status'),
        invoke<unknown>('grit_get_report', { date: requestedDate }),
      ]);
      if (requestedDate !== selectedDate) return;
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
      console.warn('Pollningsfel i GRIT Work:', err);
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
      selectWorkDate(targetDate);

      if (nextStatus.report && nextStatus.report.date === targetDate) {
        report = nextStatus.report;
      } else {
        const rawReport = await invoke<unknown>('grit_get_report', { date: targetDate });
        report = normalizeGritReport(rawReport);
      }
      dirtyDraft = false;
      notifyGritChanged();
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
    selectWorkDate(nextDate);
    reportLoading = true;
    clearFeedback();
    try {
      const rawReport = await invoke<unknown>('grit_get_report', { date: nextDate });
      report = normalizeGritReport(rawReport);
      dirtyDraft = false;
      notifyGritChanged();
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
    if (!status?.settings.ai_consent) {
      errorMessage =
        'AI-generering kräver att du först ger informerat samtycke under Inställningar (skärmbilder och loggar skickas då till din valda inloggade CLI-leverantör).';
      return;
    }
    selectedDate = targetDate;
    generating = true;
    try {
      const rawReport = await invoke<unknown>('grit_generate', { date: targetDate });
      const nextReport = normalizeGritReport(rawReport);
      if (nextReport) {
        report = nextReport;
        dirtyDraft = false;
      notifyGritChanged();
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
    notifyGritChanged();
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
      notifyGritChanged();
      await refreshStatusOnly();
      noticeMessage = 'Utkastet har sparats.';
    } catch (err) {
      errorMessage = formatError(err);
    } finally {
      savingDraft = false;
    }
  }

  async function openReporting(): Promise<void> {
    if (!report || !submitValidation.canSubmit || savingDraft) return;
    clearFeedback();
    savingDraft = true;
    try {
      const saved = normalizeGritReport(await invoke('grit_save_draft', { report }));
      if (!saved) throw new Error('Utkastet kunde inte sparas. Försök igen.');
      report = saved;
      dirtyDraft = false;
      notifyGritChanged();
      confirmSubmitOpen = true;
    } catch (err) { errorMessage = formatError(err); }
    finally { savingDraft = false; }
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
      notifyGritChanged();
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

  async function exportReport(): Promise<void> {
    if (!report) return;
    if (dirtyDraft) { errorMessage = 'Spara utkastet före export så att filen innehåller dina senaste ändringar.'; return; }
    try {
      const path = await saveDialog({ defaultPath: `grit-tidrapport-${selectedDate}.csv`, filters: [{ name: 'CSV', extensions: ['csv'] }] });
      if (!path) return;
      await invoke('grit_export_report', { date: selectedDate, targetPath: path });
      noticeMessage = 'Den sparade tidrapporten har exporterats till CSV.';
    } catch (e) { errorMessage = formatError(e); }
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

  function addManualRow(gap?: UnassignedGap): void {
    if (!report || !isReportEditable(report.status) || !status) return;
    const startSec = gap?.start ?? timeInputToUnixSeconds(report.date, '09:00');
    const endSec = gap?.end ?? timeInputToUnixSeconds(report.date, '10:00');
    const mins = gap?.minutes ?? 60;
    const newRow: GritRow = {
      id: `manual-${Date.now()}`,
      project_id: status.projects[0]?.id ?? 0,
      activity_id: 0,
      article_id: 0,
      time_code_id: 0,
      start: startSec,
      end: endSec,
      minutes: mins,
      submitted_minutes: mins,
      description: '',
      evidence_ids: [],
      needs_review: true,
      manual: true,
      manual_reason: '',
      remote_id: null,
    };
    report = { ...report, rows: [...report.rows, newRow] };
    dirtyDraft = true;
    editingRowIndex = report.rows.length - 1;
  }

  async function createManualReportAndRow(): Promise<void> {
    if (!status) return;
    clearFeedback();
    try {
      const rawReport = await invoke<unknown>('grit_create_manual_report', { date: selectedDate });
      const created = normalizeGritReport(rawReport);
      if (created) {
        report = created;
        addManualRow();
        notifyGritChanged();
      }
    } catch (err) {
      errorMessage = formatError(err);
    }
  }

  function removeRow(index: number): void {
    if (!report || !isReportEditable(report.status)) return;
    report = {
      ...report,
      rows: report.rows.filter((_, idx) => idx !== index),
    };
    dirtyDraft = true;
  }

  function openRowEditor(index: number): void {
    if (!report || !report.rows[index]) return;
    editingRowIndex = index;
  }

  function closeRowEditor(): void {
    editingRowIndex = null;
  }

  function handleRowEditorSave(e: CustomEvent<{ index: number; row: GritRow }>): void {
    updateRow(e.detail.index, e.detail.row);
    closeRowEditor();
  }

  function handleRowEditorDelete(e: CustomEvent<{ index: number }>): void {
    removeRow(e.detail.index);
    closeRowEditor();
  }

  onMount(() => {
    void loadStatusAndReport(selectedDate);
    dayTimer = setInterval(() => {
      void pollActiveReport();
    }, 15000);
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
      !(report?.date === yesterdayDate && report !== null),
  );
  $: editable = isReportEditable(report?.status);
  $: reconcilable = isReportReconcilable(report?.status);
  $: statusMeta = getStatusBadgeMeta(report?.status ?? '');
  $: submitValidation = validateReportForSubmit(report, status);
  $: providerInfo = PROVIDER_LABELS[settingsForm.provider];
  $: aiConsent = Boolean(status?.settings.ai_consent);
  $: rounding = report
    ? computeProjectRounding(report.rows, status?.projects ?? [], !isReportEditable(report.status))
    : null;
  $: unassignedGaps = report && editable ? computeUnassignedGaps(report.rows) : [];
  $: totalMinutes = rounding?.totalRecordedMinutes ?? 0;
  $: totalSubmittedMinutes = rounding?.totalSubmittedMinutes ?? 0;
  $: totalPaddingMinutes = rounding?.totalPaddingMinutes ?? 0;
  $: billableEnabled = Boolean(status?.settings.billable);
  $: totalInvoiceableHours = computeInvoiceableHours(totalSubmittedMinutes, billableEnabled);
</script>

<div class="grit-work mx-auto max-w-6xl space-y-5 px-4 py-6 sm:px-6">
  <WorkflowBar date={selectedDate} current="/report" />
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
            {status.credential_saved ? 'Spiris konfigurerat' : 'Spiris ej konfigurerad'}
          </span>
        {/if}
      </div>
    </div>

    <div class="flex flex-wrap items-center gap-2">
      <button
        type="button"
        on:click={handleSyncSpiris}
        disabled={syncingSpiris || initialLoading}
        class="inline-flex items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 disabled:opacity-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#c9d1d9] dark:hover:bg-[#30363d]"
      >
        <GritIcon name="sync" sizeClass="h-3.5 w-3.5 {syncingSpiris ? 'animate-spin' : ''}" />
        <span>{syncingSpiris ? 'Synkar Spiris...' : 'Synka Spiris'}</span>
      </button>

      <button
        type="button"
        on:click={handleArchive}
        disabled={archiving || initialLoading}
        class="inline-flex items-center gap-1.5 rounded-xl border border-slate-200 bg-white px-3.5 py-2 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 disabled:opacity-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#c9d1d9] dark:hover:bg-[#30363d]"
      >
        <GritIcon name="archive" sizeClass="h-3.5 w-3.5" />
        <span>{archiving ? 'Arkiverar...' : 'Arkivera'}</span>
      </button>


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
      Laddar GRIT Work...
    </div>
  {:else}
    <!-- Yesterday prompt -->
    {#if showYesterdayPrompt}
      <section class="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-indigo-200 bg-indigo-50/90 p-4 dark:border-indigo-800/60 dark:bg-indigo-950/30">
        <div>
          <h2 class="text-sm font-semibold text-indigo-950 dark:text-indigo-200">
            Gårdagens tidrapport ({yesterdayDate}) {status?.report?.status === 'draft' ? 'är redo för granskning' : status?.report?.status === 'running' ? 'förbereds i bakgrunden' : status?.report?.status === 'failed' ? 'behöver åtgärdas' : 'står i kö'}
          </h2>
          <p class="mt-0.5 text-xs text-indigo-800/80 dark:text-indigo-300/80">
            {aiConsent ? 'Appen förbereder förslaget automatiskt. Granska osäkerheter och komplettera luckor innan du godkänner.' : 'Aktivera AI-samtycke i inställningarna för automatisk förberedelse, eller komplettera dagen manuellt.'}
          </p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <button
            type="button"
            on:click={() => selectDate(yesterdayDate)}
            disabled={generating}
            class="rounded-xl bg-indigo-600 px-3.5 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:opacity-50"
          >
            {status?.report?.status === 'draft' ? 'Granska rapport' : 'Öppna arbetsdagen'}
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

    {#if selectedDate > yesterdayDate}<p class="text-sm text-slate-500 dark:text-[#8b949e]">Arbetsdagen pågår. Du kan analysera underlaget nu och skapa tidrapporten när dygnet är avslutat.</p>{/if}
    <!-- Date selector & pending dates -->
    <section class="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-slate-200/80 bg-white/90 p-4 shadow-sm dark:border-[#30363d] dark:bg-[#161b22]/90">
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          on:click={() => selectDate(shiftIsoDateString(selectedDate, -1))}
          class="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-slate-200 text-slate-700 hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 dark:border-[#30363d] dark:text-[#c9d1d9] dark:hover:bg-[#21262d]"
          aria-label="Föregående dag"
          title="Föregående dag"
        >
          <GritIcon name="chevron-left" sizeClass="h-4 w-4" />
        </button>
        <LocalizedDatePicker bind:value={selectedDate} max={shiftIsoDateString(yesterdayDate, 1)} label="Välj arbetsdag" on:change={() => selectDate(selectedDate)} />
        <button
          type="button"
          on:click={() => selectDate(shiftIsoDateString(selectedDate, 1))}
          class="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-slate-200 text-slate-700 hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 disabled:opacity-40 dark:border-[#30363d] dark:text-[#c9d1d9] dark:hover:bg-[#21262d]"
          disabled={selectedDate >= shiftIsoDateString(yesterdayDate, 1)}
          aria-label="Nästa dag"
          title="Nästa dag"
        >
          <GritIcon name="chevron-right" sizeClass="h-4 w-4" />
        </button>

        {#if status && status.pending_dates.length > 0}
          <div class="ml-2 flex flex-wrap items-center gap-1.5">
            <span class="text-xs text-slate-500 dark:text-[#8b949e]">Att hantera (redo / behöver granskas):</span>
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
            class="inline-flex items-center gap-1.5 rounded-xl border border-rose-200 bg-rose-50 px-3.5 py-2 text-xs font-semibold text-rose-700 transition hover:bg-rose-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-500 disabled:opacity-50 dark:border-rose-800/60 dark:bg-rose-950/40 dark:text-rose-300"
          >
            <GritIcon name="cancel" sizeClass="h-3.5 w-3.5" />
            <span>{canceling ? 'Avbryter...' : 'Avbryt generering'}</span>
          </button>
        {:else if !report || editable}
          {#if !report || report.status === 'failed'}
            <button
              type="button"
              on:click={createManualReportAndRow}
              disabled={generating || reportLoading || selectedDate > shiftIsoDateString(yesterdayDate, 1)}
              class="inline-flex items-center gap-1.5 rounded-xl border border-slate-300 bg-white px-3.5 py-2 text-xs font-semibold text-slate-800 transition hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 disabled:opacity-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#e6edf3]"
            >
              <GritIcon name="plus" sizeClass="h-3.5 w-3.5" />
              <span>Starta manuell rapport</span>
            </button>
          {/if}
          <button
            type="button"
            on:click={() => handleGenerate(selectedDate)}
            disabled={generating || reportLoading || selectedDate > yesterdayDate || !aiConsent}
            class="inline-flex items-center gap-1.5 rounded-xl px-4 py-2 text-xs font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 disabled:opacity-50 {report
              ? 'border border-slate-300 bg-white text-slate-700 hover:bg-slate-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#c9d1d9] dark:hover:bg-[#30363d]'
              : 'bg-indigo-600 text-white hover:bg-indigo-700'}"
          >
            <GritIcon name="sparkles" sizeClass="h-3.5 w-3.5" />
            <span>{report ? 'Generera om utkast' : 'Generera tidrapport'}</span>
          </button>
        {/if}
      </div>
    </section>

    {#if !aiConsent}
      <section class="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-amber-200 bg-amber-50/90 p-4 dark:border-amber-800/60 dark:bg-amber-950/30">
        <div class="max-w-3xl">
          <h2 class="text-sm font-semibold text-amber-950 dark:text-amber-200">
            Informerat samtycke krävs innan AI-generering
          </h2>
          <p class="mt-0.5 text-xs leading-relaxed text-amber-900/80 dark:text-amber-300/80">
            För att generera tidrapportutkast skickas dagens skärmbilder och aktivitetsloggar till din valda inloggade CLI-leverantör. Aktivera samtycke under Inställningar för att låsa upp generering och automatisk bakgrundsförberedelse.
          </p>
        </div>
        <button
          type="button"
          on:click={() => { window.location.hash = '/settings'; }}
          class="rounded-xl bg-amber-600 px-3.5 py-2 text-xs font-semibold text-white transition hover:bg-amber-700"
        >
          Öppna inställningar
        </button>
      </section>
    {/if}

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
          Klicka på "Generera tidrapport" för att sammanställa dagens arbete, eller lägg till en manuell lucka om du arbetat utan dator.
        </p>
        {#if selectedDate <= shiftIsoDateString(yesterdayDate, 1)}
          <div class="mt-4 flex justify-center">
            <button
              type="button"
              on:click={createManualReportAndRow}
              class="rounded-xl border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-800 transition hover:bg-slate-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#e6edf3]"
            >
              Starta manuell rapport
            </button>
          </div>
        {/if}
      </div>
    {:else}
      <!-- Report status & compact overview strip -->
      <section class="space-y-4 rounded-xl border border-slate-200/80 bg-white/90 p-5 shadow-sm dark:border-[#30363d] dark:bg-[#161b22]/90">
        <div class="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200/70 pb-3.5 dark:border-[#30363d]">
          <div class="flex flex-wrap items-center gap-2.5">
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
            {#if report.status === 'failed' || reconcilable}
              <span class="text-xs text-slate-500 dark:text-[#8b949e]">{statusMeta.description}</span>
            {/if}
          </div>

          <div class="flex flex-wrap items-center gap-5 text-xs">
            <div>
              <span class="text-slate-500 dark:text-[#8b949e]">Inspelad tid </span>
              <strong class="grit-metric font-mono text-slate-900 dark:text-[#e6edf3]">{formatMinutesSwedish(totalMinutes)}</strong>
              <span class="text-slate-400"> · {report.rows.length} {report.rows.length === 1 ? 'rad' : 'rader'}</span>
            </div>
            <div>
              <span class="text-slate-500 dark:text-[#8b949e]">Avrundad tid (Spiris) </span>
              <strong class="grit-metric font-mono text-slate-900 dark:text-[#e6edf3]">{formatMinutesSwedish(totalSubmittedMinutes)}</strong>
              {#if totalPaddingMinutes > 0}
                <span class="font-mono text-indigo-600 dark:text-indigo-400"> (+{totalPaddingMinutes} min)</span>
              {/if}
            </div>
            <div>
              <span class="text-slate-500 dark:text-[#8b949e]">Fakturerbar </span>
              <strong class="grit-metric font-mono text-slate-900 dark:text-[#e6edf3]">{formatSwedishHours(totalInvoiceableHours)}</strong>
              <span class="text-slate-400"> ({billableEnabled ? 'På' : 'Av'})</span>
            </div>
          </div>
        </div>

        {#if rounding && rounding.projectTotals.length > 0}
          <details class="rounded-lg border border-slate-200/70 bg-slate-50/60 px-3.5 py-2.5 text-xs dark:border-[#30363d] dark:bg-[#0d1117]">
            <summary class="flex cursor-pointer list-none items-center justify-between font-medium text-slate-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 dark:text-[#c9d1d9]">
              <span>Projektfördelning & avrundning ({rounding.projectTotals.length} projekt · 30 min/projekt)</span>
              <GritIcon name="chevron-down" sizeClass="h-3.5 w-3.5 text-slate-400" />
            </summary>
            <div class="mt-2.5 space-y-2 border-t border-slate-200/70 pt-2.5 dark:border-[#30363d]">
              <p class="text-[11px] text-slate-500 dark:text-[#8b949e]">
                Avrundas uppåt till närmaste 30 min per projekt och dag (påslag läggs på projektets sista rad, inte per pass).
              </p>
              <div class="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
                {#each rounding.projectTotals as proj}
                  <div class="flex items-center justify-between gap-3 rounded-lg border border-slate-200/80 bg-white px-3 py-1.5 dark:border-[#30363d] dark:bg-[#161b22]">
                    <span class="truncate font-medium text-slate-800 dark:text-[#e6edf3]">{proj.projectName}</span>
                    <span class="shrink-0 font-mono text-slate-600 dark:text-[#adbac7]">
                      {formatMinutesSwedish(proj.recordedMinutes)} → <strong class="text-slate-900 dark:text-[#e6edf3]">{formatMinutesSwedish(proj.submittedMinutes)}</strong>
                    </span>
                  </div>
                {/each}
              </div>
            </div>
          </details>
        {/if}

        {#if report.summary}
          <details class="rounded-lg border border-slate-200/70 bg-slate-50/40 px-3.5 py-2 text-xs dark:border-[#30363d] dark:bg-[#0d1117]/60">
            <summary class="flex cursor-pointer list-none items-center justify-between font-medium text-slate-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 dark:text-[#c9d1d9]">
              <span>Dagssammanfattning</span>
              <GritIcon name="chevron-down" sizeClass="h-3.5 w-3.5 text-slate-400" />
            </summary>
            <p class="grit-summary mt-2 border-t border-slate-200/70 pt-2 text-xs leading-relaxed text-slate-600 dark:border-[#30363d] dark:text-[#adbac7]">
              {report.summary}
            </p>
          </details>
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

        {#if unassignedGaps.length > 0}
          <div class="rounded-xl border border-amber-200/80 bg-amber-50/60 p-3.5 dark:border-amber-800/50 dark:bg-amber-950/20">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <span class="text-xs font-semibold text-amber-950 dark:text-amber-200">
                Ej tilldelade luckor mellan arbetspass
              </span>
              <span class="text-[11px] text-amber-900/80 dark:text-amber-300/80">
                Räknas inte automatiskt in i totaltid. Lägg till manuellt vid behov.
              </span>
            </div>
            <div class="mt-2.5 flex flex-wrap gap-2">
              {#each unassignedGaps as gap}
                <div class="inline-flex items-center gap-2 rounded-lg border border-amber-200 bg-white px-2.5 py-1.5 text-xs dark:border-amber-800/60 dark:bg-[#161b22]">
                  <span class="font-mono text-slate-800 dark:text-[#e6edf3]">
                    {unixSecondsToTimeInput(gap.start)}–{unixSecondsToTimeInput(gap.end)}
                  </span>
                  <span class="text-slate-500 dark:text-[#8b949e]">
                    ({formatMinutesSwedish(gap.minutes)})
                  </span>
                  <button
                    type="button"
                    on:click={() => addManualRow(gap)}
                    class="rounded-md bg-indigo-600 px-2 py-1 text-[11px] font-semibold text-white transition hover:bg-indigo-700"
                  >
                    Lägg till manuell tid
                  </button>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        <div class="grit-list-grid px-3.5 text-xs text-slate-500" aria-hidden="true"><span>Projekt</span><span>Timmar</span><span>Dagbok</span><span></span></div>
        <!-- Compact rows list -->
        <div class="divide-y divide-slate-200 overflow-hidden rounded-xl border border-slate-200 bg-white dark:divide-[#30363d] dark:border-[#30363d] dark:bg-[#0d1117]/60">
          {#if report.rows.length === 0}
            <div class="p-6 text-center text-sm text-slate-500 dark:text-[#8b949e]">
              Inga tidrader i denna rapport.
            </div>
          {:else}
            {#each report.rows as row, index (row.id)}
              {@const knownProject = isKnownProjectId(row.project_id, status?.projects ?? [])}
              {@const projectObj = (status?.projects ?? []).find((p) => p.id === row.project_id)}
              {@const projectLabel = projectObj ? `${projectObj.number ? projectObj.number + ' · ' : ''}${projectObj.name}` : row.project_id > 0 ? `Okänt projekt (#${row.project_id})` : 'Välj projekt'}
              {@const rowSubmittedMinutes = rounding?.rowSubmittedMinutes[index] ?? row.minutes}
              {@const rowPaddingMinutes = rounding?.rowPaddingMinutes[index] ?? 0}
              {@const startLabel = unixSecondsToTimeInput(row.start)}
              {@const endLabel = unixSecondsToTimeInput(row.end)}
              <button
                type="button"
                on:click={() => openRowEditor(index)}
                title={`${startLabel}–${endLabel} · ${row.needs_review ? 'Behöver granskas' : 'Granskad'}`}
                class="grit-row grit-list-grid w-full items-center gap-3 px-3.5 py-2.5 text-left transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-indigo-500 {editable ? 'cursor-pointer hover:bg-slate-50 dark:hover:bg-[#161b22]' : 'cursor-default'} {row.needs_review
                  ? 'bg-amber-50/50 dark:bg-amber-950/15'
                  : ''}"
              >
                <span class="grit-list-project truncate text-sm font-medium {knownProject ? 'text-slate-900' : 'text-amber-700'}">{projectLabel}</span>
                <span class="grit-list-time font-mono text-sm whitespace-nowrap">{formatSwedishHours(rowSubmittedMinutes / 60)}</span>
                <span class="grit-list-diary truncate text-sm text-slate-600">{row.description || 'Skriv en dagboksanteckning…'}</span>
                <span class="inline-flex items-center justify-end gap-2">
                  {#if row.needs_review}<span title="Behöver granskas"><GritIcon name="alert" sizeClass="h-4 w-4 text-amber-600" /></span>{/if}
                  {#if row.manual}<span title="Manuell tid"><GritIcon name="plus" sizeClass="h-3.5 w-3.5 text-indigo-500" /></span>{/if}
                  {#if row.remote_id !== null}<span title={`Rapporterad, Spiris-ID ${row.remote_id}`}><GritIcon name="check" sizeClass="h-4 w-4 text-emerald-600" /></span>{/if}
                  {#if editable}<GritIcon name="chevron-right" sizeClass="h-4 w-4 text-slate-400" />{/if}
                </span>
              </button>
            {/each}
          {/if}
        </div>

        {#if editable}
          <div class="flex justify-start">
            <button
              type="button"
              on:click={() => addManualRow()}
              class="rounded-xl border border-dashed border-slate-300 px-3.5 py-2 text-xs font-semibold text-slate-700 transition hover:bg-slate-50 dark:border-[#30363d] dark:text-[#c9d1d9] dark:hover:bg-[#21262d]"
            >
              + Lägg till manuell tid
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
    {#if !["running", "uploading"].includes(report.status)}
        <button
          type="button"
          on:click={exportReport}
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 px-3 py-1.5 text-xs font-medium text-slate-600 transition hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 dark:border-[#30363d] dark:text-[#adbac7] dark:hover:bg-[#21262d]"
        >
          <GritIcon name="download" sizeClass="h-3.5 w-3.5" />
          <span>Exportera CSV</span>
        </button>
    {/if}
          {#if editable}
            <button
              type="button"
              on:click={handleSaveDraft}
              disabled={savingDraft}
              class="rounded-xl border border-slate-300 bg-white px-4 py-2 text-xs font-semibold text-slate-800 transition hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 disabled:opacity-50 dark:border-[#30363d] dark:bg-[#21262d] dark:text-[#e6edf3]"
            >
              {savingDraft ? 'Sparar utkast...' : 'Spara utkast'}
            </button>

            <button
              type="button"
              on:click={openReporting}
              disabled={!submitValidation.canSubmit || submitting || savingDraft}
              class="inline-flex items-center gap-1.5 rounded-xl bg-emerald-600 px-4 py-2 text-xs font-semibold text-white transition hover:bg-emerald-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-500 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <GritIcon name="check" sizeClass="h-3.5 w-3.5" />
              <span>Rapportera till Spiris</span>
            </button>
          {/if}
        </div>
      </section>
    {/if}
  {/if}
</div>

{#if editingRowIndex !== null && report && report.rows[editingRowIndex] && status}
  <ReportRowEditor
    open={true}
    row={report.rows[editingRowIndex]}
    readonly={!editable}
    rowIndex={editingRowIndex}
    date={report.date}
    {status}
    on:save={handleRowEditorSave}
    on:delete={handleRowEditorDelete}
    on:close={closeRowEditor}
  />
{/if}

{#if confirmSubmitOpen && report && status}
  <ReportingDialog bind:open={confirmSubmitOpen} {report} {status} {submitting}
    on:change={(e) => { report = e.detail; dirtyDraft = true; }} on:submit={handleConfirmSubmit} />
{/if}
