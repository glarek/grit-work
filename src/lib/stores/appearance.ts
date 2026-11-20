import { writable } from 'svelte/store';

export type DashboardTheme = 'dark' | 'light';
const storageKey = 'grit-dashboard-theme';

function readTheme(): DashboardTheme {
  try { return localStorage.getItem(storageKey) === 'light' ? 'light' : 'dark'; }
  catch { return 'dark'; }
}

export const dashboardTheme = writable<DashboardTheme>(readTheme());

export function setDashboardTheme(theme: DashboardTheme): void {
  try { localStorage.setItem(storageKey, theme); } catch { /* Still apply in this window. */ }
  dashboardTheme.set(theme);
}

export function applyDashboardTheme(theme: DashboardTheme): void {
  document.documentElement.classList.toggle('dark', theme === 'dark');
  document.documentElement.dataset.gritDashboard = 'true';
  document.documentElement.style.colorScheme = theme;
}
