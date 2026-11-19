import { writable } from 'svelte/store';
import { cache } from '../../lib/stores/cache.ts';
import { getTodayIsoDate } from './gritTypes.ts';

/** One working day follows the user between evidence and reporting. */
export const workDate = writable(getTodayIsoDate());
export function notifyGritChanged(): void {
  cache.invalidate('overview');
  cache.invalidate('timeline');
  cache.invalidate('report');
  if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('grit-changed'));
}
export function selectWorkDate(date: string): void {
  if (/^\d{4}-\d{2}-\d{2}$/.test(date)) workDate.set(date);
}
