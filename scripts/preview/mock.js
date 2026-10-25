// Fake Tauri backend for rendering the UI in a plain browser (screenshots for the website).
// Loaded before the app bundle by scripts/preview/shoot.sh; never shipped.
(() => {
  const now = Math.floor(Date.now() / 1000);
  const q = new URLSearchParams(location.search);
  const game = q.get('game') === 'rs3' ? 'rs3' : 'osrs';
  const opts = { jvmArgs: '', args: '' };
  const state = {
    version: '0.1.0',
    flatpak: true,
    secretBackend: 'keyring',
    dataDir: '~/.var/app/io.github.M4S4T0_V01D.void-lodestone/data/void-lodestone',
    loginOpen: false,
    settings: {
      theme: q.get('theme') || 'void',
      javaPath: '',
      gameHome: '',
      wrapper: '',
      runelite: opts,
      hdos: opts,
      rs3: opts,
      rs3ConfigUri: '',
      minimizeOnLaunch: false,
      reduceMotion: true,
      lastGame: game,
      lastClient: game === 'rs3' ? 'rs3' : 'runelite',
      lastCharacter: 'main:1',
    },
    accounts: [
      {
        id: 'main',
        label: 'Main',
        addedAt: now,
        signedIn: true,
        characters: [
          { accountId: '1', displayName: 'Lodestar' },
          { accountId: '2', displayName: 'Iron Lodestar' },
        ],
      },
      { id: 'pure', label: 'Pure', addedAt: now, signedIn: true, characters: [{ accountId: '3', displayName: 'Rune Pure' }] },
      { id: 'alt', label: 'Skiller', addedAt: now, signedIn: false, characters: [{ accountId: '4', displayName: 'Fishy Business' }] },
    ],
    clients: [
      { client: 'runelite', installed: true, version: '2.7.6', checkedAt: now - 600, installedAt: now, size: 4_300_000 },
      { client: 'hdos', installed: true, version: 'v8', checkedAt: now - 3600, installedAt: now, size: 31_000_000 },
      { client: 'rs3', installed: false, version: '', checkedAt: 0, installedAt: 0, size: 0 },
    ],
    instances: [
      { pid: 4242, client: 'runelite', accountId: 'pure', accountLabel: 'Pure', character: 'Rune Pure', startedAt: now - 2712, log: '' },
    ],
  };
  const replies = {
    get_state: () => state,
    refresh_accounts: () => state,
    update_clients: () => state.clients,
  };
  let next = 1;
  window.__TAURI_INTERNALS__ = {
    transformCallback: () => next++,
    unregisterCallback: () => {},
    invoke: async (cmd) => (replies[cmd] ? structuredClone(replies[cmd]()) : null),
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  localStorage.setItem('lodestone.theme', state.settings.theme);
})();
