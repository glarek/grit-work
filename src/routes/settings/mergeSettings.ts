const editableKeys = [
  'auto_start', 'auto_start_silent', 'hide_dock_icon', 'lightweight_mode',
  'work_time_enabled', 'work_time_segments', 'work_start_hour', 'work_start_minute',
  'work_end_hour', 'work_end_minute', 'standard_work_hours', 'memory_enabled',
  'idle_threshold_minutes', 'daily_work_goal_minutes', 'goal_notifications',
  'ui_visual_style', 'background_image', 'background_opacity', 'background_blur',
  'break_reminder_enabled', 'break_reminder_interval_minutes',
];
const changed = (a: unknown, b: unknown) => JSON.stringify(a) !== JSON.stringify(b);
/** Apply only edits made in this form onto a fresh backend snapshot. */
export function mergeSettingsEdits<T extends object>(latest: T, draft: T, baseline: T): T {
  const next = { ...latest } as Record<string, unknown>;
  const edit = draft as Record<string, unknown>;
  const initial = baseline as Record<string, unknown>;
  for (const key of editableKeys) if (changed(edit[key], initial[key])) next[key] = edit[key];
  const privacy = { ...(next.privacy as Record<string, unknown> ?? {}) };
  const editedPrivacy = edit.privacy as Record<string, unknown> ?? {};
  const initialPrivacy = initial.privacy as Record<string, unknown> ?? {};
  for (const key of ['app_rules', 'excluded_keywords']) {
    if (changed(editedPrivacy[key], initialPrivacy[key])) privacy[key] = editedPrivacy[key] ?? [];
  }
  delete privacy.sensitive_keywords;
  next.privacy = privacy;
  return next as T;
}
