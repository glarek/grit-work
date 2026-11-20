import type { GritChoice, GritReport } from './gritTypes';
export interface ReportedRow {
  id:number; date:string; project:GritChoice|null; activity:GritChoice|null;
  time_code:GritChoice|null; article:GritChoice|null; hours:number; comment:string;
}
export interface ReportingContext {
  date:string; user:GritChoice; checked_at:number;
  projects:{project_id:number;member:boolean;active:boolean;activities:GritChoice[];time_codes:GritChoice[];articles:GritChoice[];restriction_source:'active_registries_no_project_restrictions_exposed'}[];
  existing_rows:ReportedRow[]; previous_rows:ReportedRow[];
}
/** UI readiness only. The backend repeats authoritative checks immediately before each POST. */
export function reportingChoicesValid(report:GritReport, context:ReportingContext|null):boolean {
  if (!context || context.date !== report.date || context.user.id <= 0 || context.existing_rows.length || !report.rows.length) return false;
  return report.rows.every(row => {
    const project = context.projects.find(p => p.project_id === row.project_id);
    return !!project?.member && project.active &&
      project.activities.some(c => c.id > 0 && c.id === row.activity_id) &&
      project.time_codes.some(c => c.id > 0 && c.id === row.time_code_id) &&
      project.articles.some(c => c.id > 0 && c.id === row.article_id);
  });
}
