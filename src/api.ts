import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AppState, ClientId, ClientStatus, Instance, LoginStatus, Progress, Settings } from './types';

export const api = {
  getState: () => invoke<AppState>('get_state'),
  saveSettings: (settings: Settings) => invoke<void>('save_settings', { settings }),
  addAccount: () => invoke<AppState>('add_account'),
  reauthAccount: (id: string) => invoke<AppState>('reauth_account', { id }),
  cancelLogin: () => invoke<void>('cancel_login'),
  removeAccount: (id: string) => invoke<AppState>('remove_account', { id }),
  renameAccount: (id: string, label: string) => invoke<void>('rename_account', { id, label }),
  moveAccount: (id: string, to: number) => invoke<void>('move_account', { id, to }),
  refreshAccounts: () => invoke<AppState>('refresh_accounts'),
  updateClients: (only: ClientId | null, force: boolean) => invoke<ClientStatus[]>('update_clients', { only, force }),
  launch: (client: ClientId, accountId: string, characterId: string) => invoke<Instance>('launch', { client, accountId, characterId }),
  stopInstance: (pid: number) => invoke<void>('stop_instance', { pid }),
  openFolder: (which: Folder) => invoke<void>('open_folder', { which }),
};

/** Folders the backend can open, plus the project page. */
export type Folder = 'logs' | 'data' | 'clients' | 'website';

export const events = {
  progress: (f: (p: Progress) => void): Promise<UnlistenFn> => listen<Progress>('client-progress', (e) => f(e.payload)),
  instances: (f: (i: Instance[]) => void): Promise<UnlistenFn> => listen<Instance[]>('instances', (e) => f(e.payload)),
  login: (f: (s: LoginStatus) => void): Promise<UnlistenFn> => listen<LoginStatus>('login-status', (e) => f(e.payload)),
};

/** Tauri rejects with the command's error string; anything else gets stringified. */
export function errorText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
