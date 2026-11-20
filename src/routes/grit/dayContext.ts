export interface DayContext {
  source: string;
  activities: number;
  minutes: number;
  report_status: string | null;
  projects?: { id: number; name: string; minutes: number; needs_review: boolean }[];
}
