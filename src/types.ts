export type ClientId = 'runelite' | 'hdos' | 'rs3';
export type Game = 'osrs' | 'rs3';

export interface ClientOpts {
  jvmArgs: string;
  args: string;
}

export interface Settings {
  theme: string;
  javaPath: string;
  gameHome: string;
  wrapper: string;
  runelite: ClientOpts;
  hdos: ClientOpts;
  rs3: ClientOpts;
  rs3ConfigUri: string;
  minimizeOnLaunch: boolean;
  reduceMotion: boolean;
  lastGame: string;
  lastClient: string;
  lastCharacter: string;
}

export interface Character {
  accountId: string;
  displayName: string;
}

export interface Account {
  id: string;
  label: string;
  characters: Character[];
  addedAt: number;
  signedIn: boolean;
}

export interface ClientStatus {
  client: ClientId;
  installed: boolean;
  version: string;
  checkedAt: number;
  installedAt: number;
  size: number;
}

export interface Progress {
  client: ClientId;
  phase: 'checking' | 'downloading' | 'unpacking' | 'ready' | 'error';
  received: number;
  total: number;
  message: string;
}

export interface Instance {
  pid: number;
  client: ClientId;
  accountId: string;
  accountLabel: string;
  character: string;
  startedAt: number;
  log: string;
}

export interface LoginStatus {
  phase: 'waiting' | 'verifying' | 'done' | 'error' | 'cancelled';
  message: string;
}

export interface AppState {
  version: string;
  flatpak: boolean;
  secretBackend: 'keyring' | 'file';
  settings: Settings;
  accounts: Account[];
  clients: ClientStatus[];
  instances: Instance[];
  loginOpen: boolean;
  dataDir: string;
}

/** A character chosen to play: `${account id}:${character id}`. */
export type Pick = string;

export interface ThemeColors {
  bg: string;
  panel: string;
  panelAlt: string;
  border: string;
  text: string;
  textDim: string;
  accent: string;
  accent2: string;
  success: string;
  warning: string;
  danger: string;
}

export interface Theme {
  id: string;
  name: string;
  mode: 'dark' | 'light';
  colors: ThemeColors;
}
