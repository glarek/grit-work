import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import {
  computeProjectRounding,
  computeUnassignedGaps,
  createDefaultSettings,
  normalizeGritRow,
  normalizeGritSettings,
  normalizeGritStatus,
  validateReportForSubmit,
  type GritProject,
  type GritReport,
  type GritRow,
} from './gritTypes.ts';

function makeRow(overrides: Partial<GritRow> = {}): GritRow {
  return normalizeGritRow({
    id: 'row-1',
    project_id: 1,
    activity_id: 10,
    article_id: 20,
    start: 1_700_000_000,
    end: 1_700_001_800,
    minutes: 30,
    description: 'Utveckling',
    evidence_ids: [1],
    needs_review: false,
    manual: false,
    manual_reason: '',
    remote_id: null,
    ...overrides,
  });
}

const PROJECTS: GritProject[] = [
  { id: 1, name: 'Projekt Alfa', number: '1001', active: true },
  { id: 2, name: 'Projekt Beta', number: '1002', active: true },
];

test('createDefaultSettings and normalizeGritSettings default archive_retention_days=90, ai_consent=false, use_report_history=false', () => {
  const defaults = createDefaultSettings();
  assert.equal(defaults.archive_retention_days, 90);
  assert.equal(defaults.ai_consent, false);
  assert.equal(defaults.use_report_history, false);

  const normalizedEmpty = normalizeGritSettings({});
  assert.equal(normalizedEmpty.archive_retention_days, 90);
  assert.equal(normalizedEmpty.ai_consent, false);
  assert.equal(normalizedEmpty.use_report_history, false);

  const normalizedCustom = normalizeGritSettings({
    archive_retention_days: 180,
    ai_consent: true,
    use_report_history: true,
    provider: 'agy',
  });
  assert.equal(normalizedCustom.archive_retention_days, 180);
  assert.equal(normalizedCustom.ai_consent, true);
  assert.equal(normalizedCustom.use_report_history, true);
});

test('computeProjectRounding rounds per project/day to ceil 30 min and assigns padding to last row', () => {
  const rows: GritRow[] = [
    // Project 1: 12 + 20 = 32 min -> ceil(32/30)*30 = 60 min (+28 on index 2)
    makeRow({ id: 'a1', project_id: 1, minutes: 12 }),
    // Project 2: 10 + 15 = 25 min -> ceil(25/30)*30 = 30 min (+5 on index 3)
    makeRow({ id: 'b1', project_id: 2, minutes: 10 }),
    makeRow({ id: 'a2', project_id: 1, minutes: 20 }),
    makeRow({ id: 'b2', project_id: 2, minutes: 15 }),
  ];

  const rounding = computeProjectRounding(rows, PROJECTS);

  assert.equal(rounding.totalRecordedMinutes, 57);
  assert.equal(rounding.totalSubmittedMinutes, 90);
  assert.equal(rounding.totalPaddingMinutes, 33);

  // Earlier rows keep raw minutes; last row per project receives padding
  assert.deepEqual(rounding.rowSubmittedMinutes, [12, 10, 48, 20]);
  assert.deepEqual(rounding.rowPaddingMinutes, [0, 0, 28, 5]);

  assert.deepEqual(
    rounding.projectTotals.map((p) => ({
      projectId: p.projectId,
      recordedMinutes: p.recordedMinutes,
      submittedMinutes: p.submittedMinutes,
      paddingMinutes: p.paddingMinutes,
    })),
    [
      { projectId: 1, recordedMinutes: 32, submittedMinutes: 60, paddingMinutes: 28 },
      { projectId: 2, recordedMinutes: 25, submittedMinutes: 30, paddingMinutes: 5 },
    ],
  );
});

test('validateReportForSubmit requires manual_reason for manual rows and uses rounded submittedMinutes for invoiceableHours', () => {
  const status = normalizeGritStatus({
    credential_saved: true,
    projects: PROJECTS,
    settings: {
      user_id: 1,
      time_code_id: 2,
      billable: true,
      ai_consent: true,
    },
  });

  const reportMissingReason: GritReport = {
    date: '2026-11-17',
    revision: 'rev-1',
    status: 'draft',
    summary: 'Sammanfattning',
    error: null,
    rows: [
      makeRow({ id: 'ai-1', project_id: 1, minutes: 35, needs_review: false }),
      makeRow({
        id: 'man-1',
        project_id: 1,
        minutes: 30,
        manual: true,
        manual_reason: '',
        evidence_ids: [],
        needs_review: false,
      }),
    ],
  };

  const invalid = validateReportForSubmit(reportMissingReason, status);
  assert.equal(invalid.canSubmit, false);
  assert.ok(invalid.blockers.some((b) => b.includes('manuell rad saknar')));

  const validReport: GritReport = {
    ...reportMissingReason,
    rows: [
      reportMissingReason.rows[0]!,
      { ...reportMissingReason.rows[1]!, manual_reason: 'Kundmöte utan dator' },
    ],
  };

  const valid = validateReportForSubmit(validReport, status);
  assert.equal(valid.canSubmit, true);
  assert.equal(valid.totalMinutes, 65);
  assert.equal(valid.submittedMinutes, 90);
  assert.equal(valid.invoiceableHours, 1.5);
});

test('GritSettings and GritWork UI expose consent gating, retention=90, manual gap rows, and recorded vs rounded totals', async () => {
  const [settingsSource, workSource] = await Promise.all([
    readFile(new URL('./GritSettings.svelte', import.meta.url), 'utf8'),
    readFile(new URL('./GritWork.svelte', import.meta.url), 'utf8'),
  ]);

  assert.match(settingsSource, /settingsForm\.ai_consent/);
  assert.match(settingsSource, /settingsForm\.use_report_history/);
  assert.match(settingsSource, /Använd min skrivstil från Spiris/);
  assert.match(settingsSource, /settingsForm\.archive_retention_days/);
  assert.match(settingsSource, /Informerat samtycke/);

  assert.match(workSource, /computeProjectRounding/);
  assert.match(workSource, /grit_create_manual_report/);
  assert.match(workSource, /Starta manuell rapport/);
  assert.match(workSource, /report\.status === 'failed'/);
  assert.match(workSource, /!aiConsent/);
  assert.match(workSource, /Inspelad tid/);
  assert.match(workSource, /Avrundad tid/);
  assert.match(workSource, /row\.manual_reason/);
  assert.match(workSource, /Manuell lucka/);
});

test('computeProjectRounding with preservePersisted=true keeps persisted submitted_minutes without re-rounding legacy reports', () => {
  const historicalRows: GritRow[] = [
    makeRow({ id: 'h1', project_id: 1, minutes: 17, submitted_minutes: 0 }),
    makeRow({ id: 'h2', project_id: 2, minutes: 10, submitted_minutes: 30 }),
  ];

  const preserved = computeProjectRounding(historicalRows, PROJECTS, true);
  assert.deepEqual(preserved.rowSubmittedMinutes, [17, 30]);
  assert.deepEqual(preserved.rowPaddingMinutes, [0, 20]);
  assert.equal(preserved.totalRecordedMinutes, 27);
  assert.equal(preserved.totalSubmittedMinutes, 47);
});

test('computeUnassignedGaps detects gaps strictly between first and last intervals without assuming 8h day', () => {
  const rows: GritRow[] = [
    makeRow({ id: 'r1', start: 1_700_000_000, end: 1_700_003_600, minutes: 60 }), // 00..60m
    makeRow({ id: 'r2', start: 1_700_001_800, end: 1_700_004_200, minutes: 40 }), // overlaps r1 -> ends at 70m
    makeRow({ id: 'r3', start: 1_700_006_000, end: 1_700_009_600, minutes: 60 }), // 100m..160m -> 30m gap (4200..6000)
  ];

  const gaps = computeUnassignedGaps(rows);
  assert.deepEqual(gaps, [
    { start: 1_700_004_200, end: 1_700_006_000, minutes: 30 },
  ]);

  // Single interval produces no gaps (no assumed 8h or shift start/end)
  assert.deepEqual(computeUnassignedGaps([rows[0]!]), []);
});
