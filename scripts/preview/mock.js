// Fake Tauri backend for rendering the UI in a plain browser (screenshots for the README and website).
// Loaded before the app bundle by scripts/preview/shoot.sh; never shipped. Every name in here is made up.
// Query: theme, game=rs3, click=<selector>|<selector>… (clicked in turn once the app has rendered).
(() => {
  const now = Math.floor(Date.now() / 1000);
  const q = new URLSearchParams(location.search);
  const game = q.get('game') === 'rs3' ? 'rs3' : 'osrs';
  const opts = { jvmArgs: '', args: '', jarPath: '', useJar: false, debug: false };
  const devJar = { ...opts, jarPath: '~/Documents/Projects/runelite-dev/runelite-dev.jar', useJar: true, debug: true };
  const row = (pick, client, jar) => ({ pick, client, jar });
  const state = {
    version: '0.2.0',
    flatpak: true,
    secretBackend: 'keyring',
    dataDir: '~/.var/app/io.github.M4S4T0_V01D.void-lodestone/data/void-lodestone',
    loginOpen: false,
    tray: true,
    pluginPackages: ['com.lodestar.deathrecap'],
    settings: {
      theme: q.get('theme') || 'void',
      javaPath: '',
      gameHome: '',
      wrapper: 'gamemoderun %command%',
      runelite: devJar,
      hdos: opts,
      rs3: opts,
      rs3ConfigUri: '',
      minimizeOnLaunch: false,
      reduceMotion: true,
      background: '',
      backgroundDim: 40,
      backgroundRs3: '',
      backgroundRotate: false,
      backgroundBlur: 0,
      debugPort: 5005,
      pluginProjects: '~/Documents/Projects/death-recap',
      buildTask: 'jar',
      notifications: true,
      closeToTray: true,
      groups: [
        { name: 'Bossing trio', game: 'osrs', rows: [row('main:1', 'runelite', true), row('main:2', 'runelite', false), row('pure:3', 'hdos', false)] },
        { name: 'Plugin test', game: 'osrs', rows: [row('main:2', 'runelite', true)] },
        { name: 'Clan night', game: 'rs3', rows: [row('main:1', 'rs3', false), row('pure:3', 'rs3', false)] },
      ],
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
        health: { state: 'ok', checkedAt: now - 300 },
        characters: [
          { accountId: '1', displayName: 'Lodestar' },
          { accountId: '2', displayName: 'Iron Lodestar' },
        ],
      },
      { id: 'pure', label: 'Pure', addedAt: now, signedIn: true, health: { state: 'ok', checkedAt: now - 300 }, characters: [{ accountId: '3', displayName: 'Rune Pure' }] },
      { id: 'alt', label: 'Skiller', addedAt: now, signedIn: false, characters: [{ accountId: '4', displayName: 'Fishy Business' }] },
    ],
    clients: [
      { client: 'runelite', installed: true, version: '2.7.6', checkedAt: now - 600, installedAt: now, size: 4_300_000 },
      { client: 'hdos', installed: true, version: 'v8', checkedAt: now - 3600, installedAt: now, size: 31_000_000 },
      { client: 'rs3', installed: false, version: '', checkedAt: 0, installedAt: 0, size: 0 },
    ],
    instances: [
      {
        pid: 4242,
        client: 'runelite',
        accountId: 'main',
        characterId: '2',
        accountLabel: 'Main',
        character: 'Iron Lodestar',
        startedAt: now - 1260,
        log: '',
        customJar: true,
        debugPort: 5005,
        rebuilt: true,
      },
      { pid: 4317, client: 'hdos', accountId: 'pure', characterId: '3', accountLabel: 'Pure', character: 'Rune Pure', startedAt: now - 2712, log: '' },
    ],
  };
  const day = 86400;
  const playtime = {
    'main:1': { total: 412 * 3600, sessions: [[now - 2 * day, now - 2 * day + 7200], [now - 5400, now - 600]] },
    'main:2': { total: 96 * 3600, sessions: [] },
  };
  const log = [
    '2026-10-29 19:02:11 [main] INFO  net.runelite.client.RuneLite - RuneLite 1.11.20-SNAPSHOT (developer mode) starting up',
    '2026-10-29 19:02:13 [main] INFO  n.r.client.externalplugins.ExternalPluginManager - Loading sideloaded plugin death-recap-1.3.jar',
    '2026-10-29 19:02:15 [Client] INFO  net.runelite.client.ui.ClientUI - Client initialization took 3812ms',
    '2026-10-29 19:14:40 [Client] WARN  com.lodestar.deathrecap.DeathRecapPlugin - No hitsplats recorded before death',
    '2026-10-29 19:14:40 [Client] ERROR net.runelite.client.eventbus.EventBus - Uncaught exception in event subscriber',
    'java.lang.NullPointerException: Cannot invoke "net.runelite.api.Actor.getName()" because "source" is null',
    '\tat com.lodestar.deathrecap.DeathRecapPlugin.onActorDeath(DeathRecapPlugin.java:142)',
    '\tat net.runelite.client.eventbus.EventBus$Subscriber.invoke(EventBus.java:70)',
    '\tat net.runelite.client.eventbus.EventBus.post(EventBus.java:223)',
    '\tat net.runelite.client.callback.Hooks.post(Hooks.java:212)',
    '\tat client.ab(client.java:31402)',
    '\tat client.it(client.java:28711)',
    '\tat java.base/java.lang.Thread.run(Thread.java:1583)',
    '2026-10-29 19:14:52 [Client] INFO  com.lodestar.deathrecap.DeathRecapPanel - Recap saved (12 hits, 41 damage)',
    '2026-10-29 19:20:03 [Client] INFO  net.runelite.client.plugins.loottracker.LootTrackerPlugin - Submitted 3 loot records',
  ].join('\n');
  const replies = {
    get_state: () => state,
    refresh_accounts: () => state,
    update_clients: () => state.clients,
    playtime: () => playtime,
    screenshot_list: () => [],
    // Later polls ask from the end and get nothing new.
    read_log: ({ from }) => ({ text: from ? '' : log + '\n', offset: log.length + 1, path: '…/logs/iron-lodestar-runelite.log', missing: false, running: true }),
  };
  let next = 1;
  window.__TAURI_INTERNALS__ = {
    transformCallback: () => next++,
    unregisterCallback: () => {},
    invoke: async (cmd, args) => (replies[cmd] ? structuredClone(replies[cmd](args ?? {})) : null),
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  localStorage.setItem('lodestone.theme', state.settings.theme);
  localStorage.setItem('lodestone.extras', JSON.stringify(game === 'osrs' ? [row('main:2', 'runelite', true), row('pure:3', 'hdos', false)] : []));

  const clicks = (q.get('click') || '').split('|').filter(Boolean);
  const step = (n) => {
    if (n >= clicks.length) return;
    document.querySelector(clicks[n])?.click();
    setTimeout(() => step(n + 1), 400);
  };
  if (clicks.length) addEventListener('load', () => setTimeout(() => step(0), 800));
})();
