export type ClientId = 'runelite' | 'hdos' | 'rs3';
export type Game = 'osrs' | 'rs3';

export interface ClientOpts {
  jvmArgs: string;
  args: string;
  jarPath: string;
  /** Launch with `jarPath` instead of the downloaded client (the switch next to Play). */
  useJar: boolean;
  /** With the custom jar, open a JDWP port for a debugger. */
  debug: boolean;
}

/** One row of a saved group: a character and how to start it. */
export interface GroupRow {
  pick: Pick;
  client: ClientId;
  /** Use the client's custom jar. */
  jar: boolean;
}

export interface Group {
  name: string;
  game: Game;
  rows: GroupRow[];
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
  /** A picture file behind the middle area; empty for none. */
  background: string;
  /** Percent the background is darkened. */
  backgroundDim: number;
  /** The picture on the RuneScape 3 tab; empty uses `background`. */
  backgroundRs3: string;
  /** Show a random RuneLite screenshot instead, changing every few minutes. */
  backgroundRotate: boolean;
  /** Blur in pixels. */
  backgroundBlur: number;
  /** The first JDWP port tried for debug launches. */
  debugPort: number;
  /** Plugin project folders for Build & play, one per line. */
  pluginProjects: string;
  buildTask: string;
  notifications: boolean;
  closeToTray: boolean;
  groups: Group[];
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
  /** When the saved session was made; 0 if unknown. */
  signedInAt?: number;
  signedIn: boolean;
  /** The last check of the session with Jagex, if there was one this run. */
  health?: SessionHealth | null;
}

export interface SessionHealth {
  state: 'ok' | 'expired' | 'offline';
  checkedAt: number;
}

export interface ClientStatus {
  client: ClientId;
  installed: boolean;
  version: string;
  checkedAt: number;
  installedAt: number;
  size: number;
  /** Why the last update check failed; empty if it worked. */
  lastError?: string;
  /** The version replaced by the last update, if any. */
  previousVersion?: string;
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
  /** Started with the custom jar. */
  customJar?: boolean;
  characterId: string;
  /** The jar or a sideloaded plugin changed since it started. */
  rebuilt?: boolean;
  debugPort?: number | null;
  /** For games that have ended: */
  endedAt?: number;
  exitCode?: number | null;
  crashed?: boolean;
  crashLine?: string;
}

export interface Playtime {
  total: number;
  /** [start, end] unix seconds. */
  sessions: [number, number][];
}

export interface BuildLine {
  line: string;
  err: boolean;
}

export type LogSource = 'output' | 'client';

export interface LogChunk {
  text: string;
  offset: number;
  path: string;
  missing: boolean;
  running: boolean;
}

/** Where Jagex's sign-in pages open: Lodestone's own window, or the user's browser. */
export type LoginMode = 'window' | 'browser';

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
  /** Java packages of the user's plugin projects. */
  pluginPackages: string[];
  /** A tray icon is shown. */
  tray: boolean;
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
