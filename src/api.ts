import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type {
  AppState,
  BuildLine,
  ClientId,
  ClientStatus,
  Instance,
  LogChunk,
  LoginMode,
  LoginStatus,
  LogSource,
  Playtime,
  Progress,
  Release,
  Settings,
  UpdateProgress,
} from './types';

export const api = {
  getState: () => invoke<AppState>('get_state'),
  saveSettings: (settings: Settings) => invoke<void>('save_settings', { settings }),
  addAccount: (mode: LoginMode) => invoke<AppState>('add_account', { mode }),
  reauthAccount: (id: string, mode: LoginMode) => invoke<AppState>('reauth_account', { id, mode }),
  submitLoginUrl: (url: string) => invoke<void>('submit_login_url', { url }),
  cancelLogin: () => invoke<void>('cancel_login'),
  removeAccount: (id: string) => invoke<AppState>('remove_account', { id }),
  renameAccount: (id: string, label: string) => invoke<void>('rename_account', { id, label }),
  moveAccount: (id: string, to: number) => invoke<void>('move_account', { id, to }),
  refreshAccounts: () => invoke<AppState>('refresh_accounts'),
  updateClients: (only: ClientId | null, force: boolean) => invoke<ClientStatus[]>('update_clients', { only, force }),
  /** `jar` overrides the client's jar switch; `remember` saves the choice as the last one played. */
  launch: (client: ClientId, accountId: string, characterId: string, jar?: boolean, remember = true) =>
    invoke<Instance>('launch', { client, accountId, characterId, jar: jar ?? null, remember }),
  stopInstance: (pid: number) => invoke<void>('stop_instance', { pid }),
  stopAll: () => invoke<void>('stop_all'),
  /** Start the same character again with the same client and jar, stopping it first if it runs. */
  restartInstance: (pid: number) => invoke<Instance>('restart_instance', { pid }),
  playtime: () => invoke<Record<string, Playtime>>('playtime'),
  screenshotList: () => invoke<string[]>('screenshot_list'),
  saveLog: (text: string) => invoke<string>('save_log', { text }),
  /** Build the plugin projects and copy their jars to the sideload folder; returns the jar names. */
  buildPlugins: () => invoke<string[]>('build_plugins'),
  cancelBuild: () => invoke<void>('cancel_build'),
  /** New log text from `from`; without it, the end of the log. */
  readLog: (pid: number, source: LogSource, from: number | null) => invoke<LogChunk>('read_log', { pid, source, from }),
  openFolder: (which: Folder) => invoke<void>('open_folder', { which }),
  /** A picture file's bytes. */
  backgroundImage: (path: string) => invoke<ArrayBuffer>('background_image', { path }),
  /** The latest Lodestone release, newer than this one or not. */
  checkUpdate: () => invoke<Release>('check_update'),
  /** The running version's release, for its notes. */
  releaseNotes: () => invoke<Release>('release_notes'),
  installUpdate: () => invoke<void>('install_update'),
  /** Quit and start the installed update; refused while games run. */
  restartApp: () => invoke<void>('restart_app'),
  /** Page zoom, 1 for 100%. */
  setZoom: (factor: number) => getCurrentWebview().setZoom(factor),
};

/** Folders the backend can open, plus the project page. */
export type Folder = 'logs' | 'data' | 'clients' | 'plugins' | 'screenshots' | 'website' | 'releases';

export const events = {
  progress: (f: (p: Progress) => void): Promise<UnlistenFn> => listen<Progress>('client-progress', (e) => f(e.payload)),
  instances: (f: (i: Instance[]) => void): Promise<UnlistenFn> => listen<Instance[]>('instances', (e) => f(e.payload)),
  login: (f: (s: LoginStatus) => void): Promise<UnlistenFn> => listen<LoginStatus>('login-status', (e) => f(e.payload)),
  /** The periodic session check finished; the payload names accounts that have just expired. */
  instanceEnded: (f: (i: Instance) => void): Promise<UnlistenFn> => listen<Instance>('instance-ended', (e) => f(e.payload)),
  buildOutput: (f: (l: BuildLine) => void): Promise<UnlistenFn> => listen<BuildLine>('build-output', (e) => f(e.payload)),
  /** A group was picked in the tray menu. */
  trayGroup: (f: (n: number) => void): Promise<UnlistenFn> => listen<number>('tray-group', (e) => f(e.payload)),
  updateAvailable: (f: (r: Release) => void): Promise<UnlistenFn> => listen<Release>('update-available', (e) => f(e.payload)),
  updateProgress: (f: (p: UpdateProgress) => void): Promise<UnlistenFn> => listen<UpdateProgress>('update-progress', (e) => f(e.payload)),
  sessionsChecked: (f: (lost: string[]) => void): Promise<UnlistenFn> => listen<string[]>('sessions-checked', (e) => f(e.payload)),
};

/** Tauri rejects with the command's error string; anything else gets stringified. */
export function errorText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
