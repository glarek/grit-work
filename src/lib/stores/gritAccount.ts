import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { normalizeGritStatus, type GritStatus } from '../../routes/grit/gritTypes';

export const gritAccount = writable<{ status: GritStatus | null; error: string }>({ status: null, error: '' });
let request = 0;

/** Reads the local Spiris catalogue and settings; never tests or posts remotely. */
export async function refreshGritAccount(): Promise<GritStatus | null> {
  const version = ++request;
  try {
    const status = normalizeGritStatus(await invoke<unknown>('grit_status'));
    if (version === request) gritAccount.set({ status, error: '' });
    return status;
  } catch {
    if (version === request) gritAccount.update(state => ({ ...state, error: 'Användaruppgifterna kunde inte läsas. Försök igen.' }));
    return null;
  }
}
