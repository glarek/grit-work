import { normalizeCapture, normalizeMonitors, type CaptureSettings, type CaptureMonitor } from './captureSettings.ts';
export type GritProvider = 'codex' | 'grok' | 'agy';

export type GritReportStatus =
  | 'running'
  | 'draft'
  | 'failed'
  | 'uploading'
  | 'uploaded'
  | 'unknown'
  | 'partial';

export interface GritSettings {
  archive_dir: string;
  archive_retention_days: number;
  ai_consent: boolean;
  use_report_history: boolean;
  provider: GritProvider;
  model: string;
  user_id: number;
  article_id: number;
  activity_id: number;
  time_code_id: number;
  billable: boolean;
  timeout_seconds: number;
}

export interface GritProject {
  id: number;
  name: string;
  number: string;
  active: boolean;
}

export interface GritChoice {
  id: number;
  name: string;
}

export interface GritRow {
  time_code_id?: number;
  id: string;
  project_id: number;
  activity_id: number;
  article_id: number;
  start: number;
  end: number;
  minutes: number;
  submitted_minutes: number;
  description: string;
  evidence_ids: number[];
  needs_review: boolean;
  manual: boolean;
  manual_reason: string;
  remote_id: number | null;
}

export interface GritReport {
  date: string;
  revision: string;
  status: GritReportStatus | string;
  summary: string;
  rows: GritRow[];
  error: string | null;
}

export interface GritStatus {
  capture: CaptureSettings;
  monitors: CaptureMonitor[];
  settings: GritSettings;
  local_dir: string;
  yesterday: string;
  pending_dates: string[];
  projects: GritProject[];
  users: GritChoice[];
  articles: GritChoice[];
  activities: GritChoice[];
  time_codes: GritChoice[];
  credential_saved: boolean;
  report: GritReport | null;
  last_error: string | null;
}

export interface GritArchiveResult {
  archived: number;
  message: string;
}

export interface GritCredentialsInput {
  applicationId: string;
  secret: string;
  staticToken: string;
}

export const PROVIDER_DEFAULT_MODELS: Record<GritProvider, string> = {
  codex: 'gpt-6.2-sol',
  grok: 'grok-4.7-build-fast',
  agy: 'gemini-4-hydrogen-medium',
};

export const PROVIDER_LABELS: Record<
  GritProvider,
  { name: string; cli: string; vendor: string; description: string }
> = {
  codex: {
    name: 'Codex CLI',
    cli: 'codex',
    vendor: 'OpenAI',
    description:
      'Kör lokala Codex CLI och skickar dagens insamlade aktivitetsunderlag (fönstertitlar, appar, tidsstämplar och OCR-kontext) till OpenAI.',
  },
  grok: {
    name: 'Grok CLI',
    cli: 'grok',
    vendor: 'xAI',
    description:
      'Kör lokala Grok CLI och skickar dagens insamlade aktivitetsunderlag (fönstertitlar, appar, tidsstämplar och OCR-kontext) till xAI.',
  },
  agy: {
    name: 'Antigravity CLI',
    cli: 'agy',
    vendor: 'Google Gemini',
    description:
      'Kör lokala Antigravity CLI (agy) och skickar dagens insamlade aktivitetsunderlag (fönstertitlar, appar, tidsstämplar och OCR-kontext) till Google Gemini.',
  },
};

export function createDefaultSettings(): GritSettings {
  return {
    archive_dir: '',
    archive_retention_days: 90,
    ai_consent: false,
    use_report_history: false,
    provider: 'codex',
    model: PROVIDER_DEFAULT_MODELS.codex,
    user_id: 0,
    article_id: 0,
    activity_id: 0,
    time_code_id: 0,
    billable: false,
    timeout_seconds: 600,
  };
}

export function getDefaultModelForProvider(provider: GritProvider): string {
  return PROVIDER_DEFAULT_MODELS[provider] ?? PROVIDER_DEFAULT_MODELS.codex;
}

export function applyProviderChange(
  settings: GritSettings,
  nextProvider: GritProvider,
): GritSettings {
  return {
    ...settings,
    provider: nextProvider,
    model: getDefaultModelForProvider(nextProvider),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function toFiniteNumber(value: unknown, fallback = 0): number {
  const parsed = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function toStringOr(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback;
}

export function normalizeGritSettings(raw: unknown): GritSettings {
  const defaults = createDefaultSettings();
  if (!isRecord(raw)) {
    return defaults;
  }

  const rawProvider = toStringOr(raw.provider, defaults.provider);
  const provider: GritProvider =
    rawProvider === 'codex' || rawProvider === 'grok' || rawProvider === 'agy'
      ? rawProvider
      : defaults.provider;

  const model = toStringOr(raw.model, '').trim() || getDefaultModelForProvider(provider);

  return {
    archive_dir: toStringOr(raw.archive_dir, defaults.archive_dir),
    archive_retention_days: Math.max(
      1,
      Math.trunc(toFiniteNumber(raw.archive_retention_days, defaults.archive_retention_days)),
    ),
    ai_consent: typeof raw.ai_consent === 'boolean' ? raw.ai_consent : defaults.ai_consent,
    use_report_history:
      typeof raw.use_report_history === 'boolean'
        ? raw.use_report_history
        : defaults.use_report_history,
    provider,
    model,
    user_id: Math.trunc(toFiniteNumber(raw.user_id, defaults.user_id)),
    article_id: Math.trunc(toFiniteNumber(raw.article_id, defaults.article_id)),
    activity_id: Math.trunc(toFiniteNumber(raw.activity_id, defaults.activity_id)),
    time_code_id: Math.trunc(toFiniteNumber(raw.time_code_id, defaults.time_code_id)),
    billable: typeof raw.billable === 'boolean' ? raw.billable : defaults.billable,
    timeout_seconds: Math.max(
      10,
      Math.trunc(toFiniteNumber(raw.timeout_seconds, defaults.timeout_seconds)),
    ),
  };
}

export function normalizeGritProject(raw: unknown): GritProject | null {
  if (!isRecord(raw)) return null;
  const id = Math.trunc(toFiniteNumber(raw.id, 0));
  if (id <= 0) return null;
  return {
    id,
    name: toStringOr(raw.name, `Projekt #${id}`),
    number: toStringOr(raw.number, ''),
    active: typeof raw.active === 'boolean' ? raw.active : true,
  };
}

export function normalizeGritChoice(raw: unknown): GritChoice | null {
  if (!isRecord(raw)) return null;
  const id = Math.trunc(toFiniteNumber(raw.id, 0));
  if (id <= 0) return null;
  return {
    id,
    name: toStringOr(raw.name, `#${id}`),
  };
}

export function normalizeGritRow(raw: unknown, index = 0): GritRow {
  if (!isRecord(raw)) {
    return {
      id: `row-${index + 1}`,
      project_id: 0,
      activity_id: 0,
      article_id: 0,
      time_code_id: 0,
      start: 0,
      end: 0,
      minutes: 0,
      submitted_minutes: 0,
      description: '',
      evidence_ids: [],
      needs_review: true,
      manual: false,
      manual_reason: '',
      remote_id: null,
    };
  }

  const evidence_ids = Array.isArray(raw.evidence_ids)
    ? raw.evidence_ids
        .map((item) => Math.trunc(toFiniteNumber(item, -1)))
        .filter((item) => item >= 0)
    : [];

  const remoteIdRaw = raw.remote_id;
  const remote_id =
    remoteIdRaw === null || remoteIdRaw === undefined
      ? null
      : Math.trunc(toFiniteNumber(remoteIdRaw, 0)) || null;

  return {
    id: toStringOr(raw.id, `row-${index + 1}`),
    project_id: Math.trunc(toFiniteNumber(raw.project_id, 0)),
    activity_id: Math.trunc(toFiniteNumber(raw.activity_id, 0)),
    time_code_id: Math.trunc(toFiniteNumber(raw.time_code_id, 0)),
    article_id: Math.trunc(toFiniteNumber(raw.article_id, 0)),
    start: Math.trunc(toFiniteNumber(raw.start, 0)),
    end: Math.trunc(toFiniteNumber(raw.end, 0)),
    minutes: Math.max(0, Math.trunc(toFiniteNumber(raw.minutes, 0))),
    submitted_minutes: Math.max(0, Math.trunc(toFiniteNumber(raw.submitted_minutes, 0))),
    description: toStringOr(raw.description, ''),
    evidence_ids,
    needs_review: Boolean(raw.needs_review),
    manual: Boolean(raw.manual),
    manual_reason: toStringOr(raw.manual_reason, ''),
    remote_id,
  };
}

export function normalizeGritReport(raw: unknown): GritReport | null {
  if (!isRecord(raw)) return null;
  const date = toStringOr(raw.date, '');
  if (!date) return null;

  const rows = Array.isArray(raw.rows)
    ? raw.rows.map((row, idx) => normalizeGritRow(row, idx))
    : [];

  return {
    date,
    revision: toStringOr(raw.revision, ''),
    status: toStringOr(raw.status, 'draft'),
    summary: toStringOr(raw.summary, ''),
    rows,
    error: typeof raw.error === 'string' && raw.error.trim() ? raw.error : null,
  };
}

export function normalizeGritStatus(raw: unknown): GritStatus {
  if (!isRecord(raw)) {
    return {
      capture: normalizeCapture(null),
      monitors: [],
      settings: createDefaultSettings(),
      local_dir: '',
      yesterday: getYesterdayIsoDate(),
      pending_dates: [],
      projects: [],
      users: [],
      articles: [],
      activities: [],
      time_codes: [],
      credential_saved: false,
      report: null,
      last_error: null,
    };
  }

  return {
    capture: normalizeCapture(raw.capture),
    monitors: normalizeMonitors(raw.monitors),
    settings: normalizeGritSettings(raw.settings),
    local_dir: toStringOr(raw.local_dir, ''),
    yesterday: toStringOr(raw.yesterday, getYesterdayIsoDate()),
    pending_dates: Array.isArray(raw.pending_dates)
      ? raw.pending_dates.filter((d): d is string => typeof d === 'string' && d.trim().length > 0)
      : [],
    projects: Array.isArray(raw.projects)
      ? raw.projects.map(normalizeGritProject).filter((p): p is GritProject => p !== null)
      : [],
    users: Array.isArray(raw.users)
      ? raw.users.map(normalizeGritChoice).filter((c): c is GritChoice => c !== null)
      : [],
    articles: Array.isArray(raw.articles)
      ? raw.articles.map(normalizeGritChoice).filter((c): c is GritChoice => c !== null)
      : [],
    activities: Array.isArray(raw.activities)
      ? raw.activities.map(normalizeGritChoice).filter((c): c is GritChoice => c !== null)
      : [],
    time_codes: Array.isArray(raw.time_codes)
      ? raw.time_codes.map(normalizeGritChoice).filter((c): c is GritChoice => c !== null)
      : [],
    credential_saved: Boolean(raw.credential_saved),
    report: normalizeGritReport(raw.report),
    last_error:
      typeof raw.last_error === 'string' && raw.last_error.trim() ? raw.last_error : null,
  };
}

/**
 * UI never edits uploaded, uploading, unknown, partial, or running reports.
 */
export function isReportEditable(status: string | null | undefined): boolean {
  if (!status) return false;
  return status === 'draft' || status === 'failed';
}

/**
 * Reports in unknown, partial, or uploading state use grit_reconcile ("Kontrollera i Spiris")
 * instead of blind retry.
 */
export function isReportReconcilable(status: string | null | undefined): boolean {
  if (!status) return false;
  return status === 'unknown' || status === 'partial' || status === 'uploading';
}

export function isKnownProjectId(
  projectId: number,
  projects: readonly GritProject[],
): boolean {
  return projectId > 0 && projects.some((p) => p.id === projectId);
}

/**
 * Spiris invoiceableHours: minutes / 60 if billable === true, else 0.
 */
export function computeInvoiceableHours(minutes: number, billable: boolean): number {
  if (!billable || !Number.isFinite(minutes) || minutes <= 0) {
    return 0;
  }
  return minutes / 60;
}

export function formatSwedishHours(hours: number): string {
  if (!Number.isFinite(hours) || hours <= 0) {
    return '0,00 h';
  }
  return `${hours.toFixed(2).replace('.', ',')} h`;
}

export function formatMinutesSwedish(totalMinutes: number): string {
  const safe = Math.max(0, Math.round(totalMinutes || 0));
  const hours = Math.floor(safe / 60);
  const mins = safe % 60;
  if (hours === 0) {
    return `${mins} min`;
  }
  if (mins === 0) {
    return `${hours} h`;
  }
  return `${hours} h ${mins} min`;
}

export interface ProjectRoundingSummary {
  projectId: number;
  projectName: string;
  recordedMinutes: number;
  submittedMinutes: number;
  paddingMinutes: number;
  rowCount: number;
}

export interface ReportRoundingResult {
  rowSubmittedMinutes: number[];
  rowPaddingMinutes: number[];
  projectTotals: ProjectRoundingSummary[];
  totalRecordedMinutes: number;
  totalSubmittedMinutes: number;
  totalPaddingMinutes: number;
}

/**
 * Deterministic per-project/day 30-minute ceiling rounding:
 * submitted_minutes = ceil(sum(raw minutes) / 30) * 30 per project,
 * with padding assigned to the LAST row for that project.
 *
 * When `preservePersisted` is true (uploaded/unknown/partial/uploading historical reports),
 * persisted `row.submitted_minutes` (or `row.minutes` for legacy rows with 0) is used
 * without re-rounding.
 */
export function computeProjectRounding(
  rows: readonly GritRow[],
  projects: readonly GritProject[],
  preservePersisted = false,
): ReportRoundingResult {
  const rowSubmittedMinutes = rows.map((r) => Math.max(0, Math.trunc(r.minutes || 0)));
  const rowPaddingMinutes = rows.map(() => 0);
  const projectGroups = new Map<
    number,
    { indices: number[]; recordedMinutes: number; persistedSubmittedMinutes: number }
  >();

  rows.forEach((row, index) => {
    const minutes = Math.max(0, Math.trunc(row.minutes || 0));
    const persisted =
      row.submitted_minutes > 0 ? Math.max(0, Math.trunc(row.submitted_minutes)) : minutes;
    const existing = projectGroups.get(row.project_id);
    if (existing) {
      existing.indices.push(index);
      existing.recordedMinutes += minutes;
      existing.persistedSubmittedMinutes += persisted;
    } else {
      projectGroups.set(row.project_id, {
        indices: [index],
        recordedMinutes: minutes,
        persistedSubmittedMinutes: persisted,
      });
    }
    if (preservePersisted) {
      rowSubmittedMinutes[index] = persisted;
      rowPaddingMinutes[index] = Math.max(0, persisted - minutes);
    }
  });

  const projectTotals: ProjectRoundingSummary[] = [];
  let totalRecordedMinutes = 0;
  let totalSubmittedMinutes = 0;

  for (const [projectId, group] of projectGroups.entries()) {
    const recordedMinutes = group.recordedMinutes;
    const submittedMinutes = preservePersisted
      ? group.persistedSubmittedMinutes
      : recordedMinutes > 0
        ? Math.ceil(recordedMinutes / 30) * 30
        : 0;
    const paddingMinutes = Math.max(0, submittedMinutes - recordedMinutes);

    if (!preservePersisted) {
      const lastIndex = group.indices[group.indices.length - 1];
      if (lastIndex !== undefined && paddingMinutes > 0) {
        rowSubmittedMinutes[lastIndex] = (rowSubmittedMinutes[lastIndex] ?? 0) + paddingMinutes;
        rowPaddingMinutes[lastIndex] = paddingMinutes;
      }
    }

    const project = projects.find((p) => p.id === projectId);
    const projectName = project
      ? `${project.number ? `${project.number} · ` : ''}${project.name}`
      : projectId > 0
        ? `Projekt #${projectId}`
        : 'Inget projekt valt';

    projectTotals.push({
      projectId,
      projectName,
      recordedMinutes,
      submittedMinutes,
      paddingMinutes,
      rowCount: group.indices.length,
    });

    totalRecordedMinutes += recordedMinutes;
    totalSubmittedMinutes += submittedMinutes;
  }

  return {
    rowSubmittedMinutes,
    rowPaddingMinutes,
    projectTotals,
    totalRecordedMinutes,
    totalSubmittedMinutes,
    totalPaddingMinutes: totalSubmittedMinutes - totalRecordedMinutes,
  };
}

export interface UnassignedGap {
  start: number;
  end: number;
  minutes: number;
}

/**
 * Finds unassigned gaps strictly between the first and last recorded/manual intervals
 * (no assumed shift start/end or 8h day). Gaps are never auto-counted into totals.
 */
export function computeUnassignedGaps(rows: readonly GritRow[]): UnassignedGap[] {
  const intervals = rows
    .filter((r) => r.start > 0 && r.end > r.start)
    .map((r) => ({ start: r.start, end: r.end }))
    .sort((a, b) => a.start - b.start || a.end - b.end);

  if (intervals.length < 2) {
    return [];
  }

  const gaps: UnassignedGap[] = [];
  let cursor = intervals[0]!.end;

  for (let i = 1; i < intervals.length; i += 1) {
    const current = intervals[i]!;
    if (current.start > cursor) {
      const minutes = Math.floor((current.start - cursor) / 60);
      if (minutes >= 1) {
        gaps.push({
          start: cursor,
          end: current.start,
          minutes,
        });
      }
    }
    if (current.end > cursor) {
      cursor = current.end;
    }
  }

  return gaps;
}

export interface SubmitValidationResult {
  canSubmit: boolean;
  blockers: string[];
  totalMinutes: number;
  submittedMinutes: number;
  invoiceableHours: number;
  unresolvedReviewCount: number;
  invalidProjectCount: number;
}

export function validateReportForSubmit(
  report: GritReport | null,
  status: GritStatus | null,
): SubmitValidationResult {
  const blockers: string[] = [];

  if (!report || !status) {
    return {
      canSubmit: false,
      blockers: ['Ingen tidrapport är laddad.'],
      totalMinutes: 0,
      submittedMinutes: 0,
      invoiceableHours: 0,
      unresolvedReviewCount: 0,
      invalidProjectCount: 0,
    };
  }

  const rounding = computeProjectRounding(
    report.rows,
    status.projects,
    !isReportEditable(report.status),
  );
  const totalMinutes = rounding.totalRecordedMinutes;
  const submittedMinutes = rounding.totalSubmittedMinutes;
  const invoiceableHours = computeInvoiceableHours(submittedMinutes, status.settings.billable);
  const unresolvedReviewCount = report.rows.filter((r) => r.needs_review).length;
  const invalidProjectCount = report.rows.filter(
    (r) => !isKnownProjectId(r.project_id, status.projects),
  ).length;
  const zeroMinuteCount = report.rows.filter((r) => (r.minutes || 0) <= 0).length;
  const emptyDescriptionCount = report.rows.filter((r) => !r.description.trim()).length;
  const missingManualReasonCount = report.rows.filter(
    (r) => r.manual && !r.manual_reason.trim(),
  ).length;

  if (report.status !== 'draft') {
    if (report.status === 'uploaded') {
      blockers.push('Tidrapporten är redan skickad till Spiris.');
    } else if (isReportReconcilable(report.status)) {
      blockers.push(
        'Överföringsstatus är oklar eller pågående. Använd "Kontrollera i Spiris" istället för att skicka på nytt.',
      );
    } else if (report.status === 'running') {
      blockers.push('AI-generering pågår.');
    } else {
      blockers.push('Spara rapporten som ett giltigt utkast innan den skickas.');
    }
  }

  if (!status.credential_saved) {
    blockers.push('Spiris-autentiseringsuppgifter saknas. Spara dem under Inställningar.');
  }

  if (status.settings.user_id <= 0) {
    blockers.push('Ingen Spiris-användare är vald i inställningarna.');
  }

  if (report.rows.length === 0) {
    blockers.push('Tidrapporten innehåller inga rader.');
  }

  if (invalidProjectCount > 0) {
    blockers.push(
      `${invalidProjectCount} ${invalidProjectCount === 1 ? 'rad saknar' : 'rader saknar'} ett giltigt projekt från Spiris.`,
    );
  }

  if (unresolvedReviewCount > 0) {
    blockers.push(
      `${unresolvedReviewCount} ${unresolvedReviewCount === 1 ? 'osäker rad måste' : 'osäkra rader måste'} granskas och klarmarkeras.`,
    );
  }

  if (zeroMinuteCount > 0) {
    blockers.push(
      `${zeroMinuteCount} ${zeroMinuteCount === 1 ? 'rad har' : 'rader har'} 0 minuter.`,
    );
  }

  if (emptyDescriptionCount > 0) {
    blockers.push(
      `${emptyDescriptionCount} ${emptyDescriptionCount === 1 ? 'rad saknar' : 'rader saknar'} beskrivning.`,
    );
  }

  if (missingManualReasonCount > 0) {
    blockers.push(
      `${missingManualReasonCount} ${missingManualReasonCount === 1 ? 'manuell rad saknar' : 'manuella rader saknar'} orsak/anledning.`,
    );
  }

  if (!report.revision.trim()) {
    blockers.push('Utkastet saknar revision. Spara utkastet först.');
  }

  return {
    canSubmit: blockers.length === 0,
    blockers,
    totalMinutes,
    submittedMinutes,
    invoiceableHours,
    unresolvedReviewCount,
    invalidProjectCount,
  };
}

export function unixSecondsToTimeInput(unixSeconds: number): string {
  if (!Number.isFinite(unixSeconds) || unixSeconds <= 0) {
    return '09:00';
  }
  const date = new Date(unixSeconds * 1000);
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  return `${hours}:${minutes}`;
}

export function timeInputToUnixSeconds(isoDate: string, timeStr: string): number {
  const dateMatch = /^(\d{4})-(\d{2})-(\d{2})$/.exec(isoDate.trim());
  const timeMatch = /^(\d{1,2}):(\d{2})$/.exec(timeStr.trim());
  if (!dateMatch || !timeMatch) {
    return 0;
  }
  const year = Number(dateMatch[1]);
  const month = Number(dateMatch[2]);
  const day = Number(dateMatch[3]);
  const hour = Number(timeMatch[1]);
  const minute = Number(timeMatch[2]);
  if (hour > 23 || minute > 59) return 0;
  const dt = new Date(year, month - 1, day, hour, minute, 0, 0);
  if (dt.getFullYear() !== year || dt.getMonth() !== month - 1 || dt.getDate() !== day || dt.getHours() !== hour || dt.getMinutes() !== minute) return 0;
  return Math.floor(dt.getTime() / 1000);
}

export function getTodayIsoDate(): string {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, '0');
  const d = String(now.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}

export function shiftIsoDateString(isoDate: string, deltaDays: number): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(isoDate.trim());
  if (!match) return getTodayIsoDate();
  const dt = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]), 12, 0, 0);
  dt.setDate(dt.getDate() + deltaDays);
  const y = dt.getFullYear();
  const m = String(dt.getMonth() + 1).padStart(2, '0');
  const d = String(dt.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}

export function getYesterdayIsoDate(): string {
  return shiftIsoDateString(getTodayIsoDate(), -1);
}

export function getStatusBadgeMeta(status: string): {
  label: string;
  description: string;
  tone: 'neutral' | 'info' | 'warning' | 'success' | 'danger';
} {
  switch (status) {
    case 'running':
      return {
        label: 'Genererar...',
        description: 'AI-motorn analyserar dagens aktivitetsunderlag och bygger ett utkast.',
        tone: 'info',
      };
    case 'draft':
      return {
        label: 'Utkast',
        description: 'Utkastet är redo för granskning, redigering och godkännande.',
        tone: 'neutral',
      };
    case 'failed':
      return {
        label: 'Misslyckades',
        description: 'Ett fel uppstod. Kontrollera felmeddelandet nedan.',
        tone: 'danger',
      };
    case 'uploading':
      return {
        label: 'Skickar till Spiris...',
        description: 'Tidrader överförs till Spiris. Redigering är låst.',
        tone: 'info',
      };
    case 'uploaded':
      return {
        label: 'Skickad till Spiris',
        description: 'Tidrapporten är överförd och låst mot ändringar.',
        tone: 'success',
      };
    case 'unknown':
      return {
        label: 'Okänd överföringsstatus',
        description:
          'Svaret från Spiris kunde inte verifieras. Kontrollera status i Spiris innan ny åtgärd.',
        tone: 'warning',
      };
    case 'partial':
      return {
        label: 'Delvis överförd',
        description:
          'Vissa rader har registrerats i Spiris medan andra saknar bekräftelse. Kontrollera i Spiris.',
        tone: 'warning',
      };
    default:
      return {
        label: status || 'Ej skapad',
        description: 'Ingen rapport genererad för detta datum ännu.',
        tone: 'neutral',
      };
  }
}
