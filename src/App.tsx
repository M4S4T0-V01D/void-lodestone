import { useCallback, useEffect, useRef, useState } from 'react';
import { AlertTriangle, ArrowUpCircle, CheckCircle2, Settings as Cog, PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen, X } from 'lucide-react';
import { api, errorText, events, type Folder } from './api';
import logo from './assets/logo.svg';
import { makeBackground, type Background } from './background';
import Accounts from './components/Accounts';
import BuildModal, { type Build } from './components/BuildModal';
import Instances from './components/Instances';
import LoginChoice from './components/LoginChoice';
import LoginOverlay from './components/LoginOverlay';
import LogViewer from './components/LogViewer';
import SettingsModal from './components/SettingsModal';
import Stage from './components/Stage';
import UpdateModal from './components/UpdateModal';
import { characterName, CLIENTS, loadRows, resolvePick, splitPick, validPick, type Row } from './format';
import { applyTheme, findTheme, withAccent } from './themes';
import type { AppState, ClientId, Game, Group, Instance, LoginMode, LoginStatus, Pick, Playtime, Progress, Release, Settings, UpdateProgress } from './types';

/** `crashed`: the game that crashed, for the Open log and Restart buttons. */
type Notice = { kind: 'ok' | 'error' | 'warn'; text: string; crashed?: Instance };

function crashText(i: Instance): string {
  const why = i.crashLine ? `: ${i.crashLine}` : i.exitCode ? ` (exit code ${i.exitCode}).` : ' within a minute of starting.';
  return `${i.character} (${CLIENTS[i.client].name}) crashed${why}`;
}

/** The appearance settings that apply to the whole window: colours, motion, corners and zoom. */
function useLook(s: Settings | undefined) {
  const theme = s?.theme;
  const accent = s?.accent ?? '';
  useEffect(() => {
    if (!theme) return;
    applyTheme(withAccent(findTheme(theme), accent));
    localStorage.setItem('lodestone.theme', theme);
    localStorage.setItem('lodestone.accent', accent);
  }, [theme, accent]);
  const reduceMotion = !!s?.reduceMotion;
  useEffect(() => {
    document.documentElement.classList.toggle('reduce-motion', reduceMotion);
  }, [reduceMotion]);
  const radius = s?.radius ?? 10;
  useEffect(() => {
    document.documentElement.style.setProperty('--radius', `${Math.min(20, Math.max(0, radius))}px`);
  }, [radius]);
  const scale = s?.uiScale ?? 100;
  useEffect(() => {
    if (!s) return;
    api.setZoom(Math.min(140, Math.max(80, scale)) / 100).catch(() => {});
  }, [scale, !!s]); // eslint-disable-line react-hooks/exhaustive-deps
}

export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [fatal, setFatal] = useState('');
  const [progress, setProgress] = useState<Partial<Record<ClientId, Progress>>>({});
  const [game, setGame] = useState<Game>('osrs');
  const [osrsClient, setOsrsClient] = useState<ClientId>('runelite');
  const [pick, setPick] = useState<Pick>('');
  const [launching, setLaunching] = useState(false);
  const [busy, setBusy] = useState(false);
  const [login, setLogin] = useState<LoginStatus | null>(null);
  const [signingIn, setSigningIn] = useState(false);
  const [loginMode, setLoginMode] = useState<LoginMode>('window');
  /** Set while the Browser/Window chooser is open: the account to sign in again, or null for a new one. */
  const [choosing, setChoosing] = useState<{ id?: string } | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  /** Extra character rows under the main one, kept between runs. */
  const [extras, setExtras] = useState<Row[]>(() => {
    try {
      return loadRows(JSON.parse(localStorage.getItem('lodestone.extras') ?? '[]'));
    } catch {
      return [];
    }
  });
  /** The picture file's bytes, and what's drawn from them. */
  const [picture, setPicture] = useState<Blob | null>(null);
  const [background, setBackground] = useState<Background | null>(null);
  /** Appearance settings shown while the Settings window is open, before they're saved. */
  const [look, setLook] = useState<Partial<Settings> | null>(null);
  const [logOf, setLogOf] = useState<{ inst: Instance; atError: boolean } | null>(null);
  const [playtime, setPlaytime] = useState<Record<string, Playtime>>({});
  const [restarting, setRestarting] = useState<number[]>([]);
  const [build, setBuild] = useState<Build | null>(null);
  const [shots, setShots] = useState<string[]>([]);
  const [shot, setShot] = useState(0);
  /** Starts a saved group; the tray's menu calls it. Set on every render so it sees the current state. */
  const playGroupRef = useRef<(n: number) => void>(() => {});
  /** What the keyboard shortcuts do, set on every render like playGroupRef. */
  const keysRef = useRef<{ play: () => void; settings: () => void }>({ play: () => {}, settings: () => {} });
  /** A newer Lodestone release, and the dialog showing it (or the running version's notes). */
  const [update, setUpdate] = useState<Release | null>(null);
  const [updateView, setUpdateView] = useState<{ release: Release; mode: 'update' | 'news' } | null>(null);
  const [updateProgress, setUpdateProgress] = useState<UpdateProgress | null>(null);

  const shown: Settings | undefined = state ? (look ? { ...state.settings, ...look } : state.settings) : undefined;
  useLook(shown);

  const fail = useCallback((e: unknown) => setNotice({ kind: 'error', text: errorText(e) }), []);

  useEffect(() => {
    let alive = true;
    api
      .getState()
      .then((s) => {
        if (!alive) return;
        setState(s);
        setGame(s.settings.lastGame === 'rs3' ? 'rs3' : 'osrs');
        setOsrsClient(s.settings.lastClient === 'hdos' ? 'hdos' : 'runelite');
        setPick(resolvePick(s.settings.lastCharacter, s.accounts));
        setUpdate(s.update);
        // The first start of a new version shows its release notes once (not on a fresh install).
        if (s.settings.seenVersion !== s.version) {
          if (s.settings.seenVersion || s.accounts.length)
            api
              .releaseNotes()
              .then((release) => alive && release.notes.trim() && setUpdateView((v) => v ?? { release, mode: 'news' }))
              .catch(() => {});
          const settings = { ...s.settings, seenVersion: s.version };
          setState((old) => old && { ...old, settings: { ...old.settings, seenVersion: s.version } });
          api.saveSettings(settings).catch(() => {});
        }
        // Check the saved sessions in the background so expired ones show up before you press Play.
        if (s.accounts.length)
          api
            .refreshAccounts()
            .then((n) => alive && setState(n))
            .catch(() => {});
      })
      .catch((e) => alive && setFatal(errorText(e)));
    api
      .playtime()
      .then((p) => alive && setPlaytime(p))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    const subs = [
      events.progress((p) => {
        setProgress((old) => ({ ...old, [p.client]: p }));
        // A failed check is recorded on the client, so refresh after errors too.
        if (p.phase === 'ready' || p.phase === 'error')
          api
            .getState()
            .then((s) => setState((old) => (old ? { ...old, clients: s.clients } : s)))
            .catch(() => {});
      }),
      events.instances((instances) => setState((old) => (old ? { ...old, instances } : old))),
      events.login(setLogin),
      events.instanceEnded((inst) => {
        api
          .playtime()
          .then(setPlaytime)
          .catch(() => {});
        if (inst.crashed) setNotice({ kind: 'error', text: crashText(inst), crashed: inst });
      }),
      events.buildOutput((l) => setBuild((old) => old && { ...old, lines: [...old.lines, l].slice(-5000) })),
      events.trayGroup((n) => playGroupRef.current(n)),
      events.updateAvailable(setUpdate),
      events.updateProgress(setUpdateProgress),
      events.sessionsChecked((lost) => {
        api
          .getState()
          .then((s) => setState((old) => (old ? { ...old, accounts: s.accounts } : s)))
          .catch(() => {});
        if (lost.length)
          setNotice({
            kind: 'warn',
            text: `${lost.length === 1 ? `${lost[0]}'s session has` : `Sessions for ${lost.join(', ')} have`} expired. Use “Sign in again” in the accounts list.`,
          });
      }),
    ];
    return () => subs.forEach((p) => p.then((off) => off()));
  }, []);

  useEffect(() => {
    localStorage.setItem('lodestone.extras', JSON.stringify(extras));
  }, [extras]);

  // Ctrl+Enter plays, Ctrl+, opens Settings; not while a dialog is open.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.altKey || document.querySelector('.overlay')) return;
      if (e.key === 'Enter') keysRef.current.play();
      else if (e.key === ',') keysRef.current.settings();
      else return;
      e.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // The rotating background: RuneLite's screenshots, on the Old School tab.
  const rotate = !!state?.settings.backgroundRotate && game === 'osrs';
  const rotateMinutes = Math.max(1, state?.settings.rotateMinutes || 5);
  useEffect(() => {
    if (!rotate) return;
    let alive = true;
    api
      .screenshotList()
      .then((list) => {
        if (!alive) return;
        setShots(list);
        setShot(Math.floor(Math.random() * list.length));
      })
      .catch(() => {});
    const t = setInterval(() => setShot((n) => n + 1 + Math.floor(Math.random() * 7)), rotateMinutes * 60_000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [rotate, rotateMinutes]);

  const rs3Bg = state?.settings.backgroundRs3.trim() ?? '';
  const bgPath = rotate && shots.length ? shots[shot % shots.length]! : game === 'rs3' && rs3Bg ? rs3Bg : (state?.settings.background.trim() ?? '');
  useEffect(() => {
    let alive = true;
    if (!bgPath) {
      setPicture(null);
      return;
    }
    api
      .backgroundImage(bgPath)
      .then((bytes) => alive && setPicture(new Blob([bytes])))
      .catch((e) => {
        if (!alive) return;
        // A screenshot that went away: try another next time.
        if (rotate && shots.length) return;
        setPicture(null);
        setNotice({ kind: 'warn', text: `Background picture: ${errorText(e)}` });
      });
    return () => {
      alive = false;
    };
  }, [bgPath, rotate, shots.length]);

  // Draw the picture, blurred ahead of time. Waits a moment so dragging the blur slider doesn't redraw on every step.
  const blur = Math.min(30, Math.max(0, shown?.backgroundBlur ?? 0));
  useEffect(() => {
    if (!picture) {
      setBackground(null);
      return;
    }
    let alive = true;
    const t = setTimeout(() => {
      const el = document.querySelector('.stage');
      makeBackground(picture, blur, { width: el?.clientWidth ?? window.innerWidth, height: el?.clientHeight ?? window.innerHeight })
        .then((b) => (alive ? setBackground(b) : URL.revokeObjectURL(b.url)))
        .catch(() => {});
    }, 120);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [picture, blur]);
  // Let the old picture go once the new one has faded in.
  const shownUrl = useRef('');
  useEffect(() => {
    const old = shownUrl.current;
    shownUrl.current = background?.url ?? '';
    if (old && old !== shownUrl.current) setTimeout(() => URL.revokeObjectURL(old), 1000);
  }, [background]);

  useEffect(() => {
    if (notice?.kind !== 'ok') return;
    const t = setTimeout(() => setNotice(null), 6000);
    return () => clearTimeout(t);
  }, [notice]);

  if (fatal)
    return (
      <div className="empty" style={{ paddingTop: 80 }}>
        <AlertTriangle /> <div>Lodestone couldn't start: {fatal}</div>
      </div>
    );
  if (!state || !shown) return <div className="app" />;

  const client: ClientId = game === 'rs3' ? 'rs3' : osrsClient;
  const current = resolvePick(pick, state.accounts);
  const hasJar = (c: ClientId) => c !== 'rs3' && !!state.settings[c].jarPath.trim();
  // Rows whose character has gone (account removed) show as unchosen.
  const extraRows = extras.map((x) => (validPick(x.pick, state.accounts) ? x : { ...x, pick: '' }));

  /** What Play starts: the main row with the chosen client and jar switch, then each extra row. */
  const rowsNow = (): Row[] => {
    const rows: Row[] = [{ pick: current, client, jar: hasJar(client) && state.settings[client].useJar }];
    for (const x of extraRows) {
      const c: ClientId = game === 'rs3' ? 'rs3' : x.client === 'rs3' ? 'runelite' : x.client;
      if (x.pick && !rows.some((y) => y.pick === x.pick)) rows.push({ pick: x.pick, client: c, jar: x.jar && hasJar(c) });
    }
    return rows.filter((x) => x.pick);
  };

  const nameOf = (k: Pick) => {
    const parts = splitPick(k);
    const acc = state.accounts.find((a) => a.id === parts?.[0]);
    return characterName(acc?.characters.find((c) => c.accountId === parts?.[1])?.displayName ?? '');
  };

  const persist = (settings: Settings) => {
    setState((old) => old && { ...old, settings });
    api.saveSettings(settings).catch(fail);
  };

  const signIn = async (mode: LoginMode, id?: string) => {
    const before = new Set(state.accounts.map((a) => a.id));
    setChoosing(null);
    setLoginMode(mode);
    setSigningIn(true);
    setLogin({ phase: 'waiting', message: '' });
    try {
      const s = id ? await api.reauthAccount(id, mode) : await api.addAccount(mode);
      setState(s);
      const added = s.accounts.find((a) => !before.has(a.id) && a.characters.length);
      if (added) setPick(`${added.id}:${added.characters[0]!.accountId}`);
      setNotice({ kind: 'ok', text: id ? 'Signed in again.' : 'Account added.' });
    } catch (e) {
      if (errorText(e) !== 'cancelled') fail(e);
    } finally {
      setSigningIn(false);
      setLogin(null);
    }
  };

  const withBusy = async (f: () => Promise<void>) => {
    setBusy(true);
    try {
      await f();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  /**
   * Start each row in turn, a little apart so the clients don't all load at once. `remember` saves the first row as
   * the last one played; `restart` restarts characters that already run instead of starting a second copy.
   */
  const startRows = async (rows: Row[], { remember = false, restart = false } = {}) => {
    setLaunching(true);
    setNotice(null);
    const started: string[] = [];
    const failed: string[] = [];
    for (const [n, row] of rows.entries()) {
      const parts = splitPick(row.pick);
      if (!parts) continue;
      const name = nameOf(row.pick);
      if (n) await new Promise((res) => setTimeout(res, 2500));
      try {
        const running = restart && state.instances.find((i) => i.accountId === parts[0] && i.characterId === parts[1]);
        if (running) await api.restartInstance(running.pid);
        else await api.launch(row.client, parts[0], parts[1], row.jar, remember && n === 0);
        started.push(rows.length > 1 && new Set(rows.map((x) => x.client)).size > 1 ? `${name} (${CLIENTS[row.client].name})` : name);
      } catch (e) {
        failed.push(`${name}: ${errorText(e)}`);
      }
    }
    setLaunching(false);
    const what = new Set(rows.map((x) => x.client)).size === 1 ? `${CLIENTS[rows[0]!.client].name} started` : 'Started';
    if (failed.length) {
      setNotice({ kind: 'error', text: `${started.length ? `Started ${started.join(', ')}. ` : ''}Couldn't start ${failed.join('; ')}` });
      // A failed launch may have found an expired session.
      api
        .getState()
        .then(setState)
        .catch(() => {});
    } else if (started.length) setNotice({ kind: 'ok', text: `${what} for ${started.join(', ')}.` });
    if (remember && started.length)
      setState(
        (old) =>
          old && {
            ...old,
            settings: { ...old.settings, lastGame: game, lastClient: game === 'osrs' ? client : old.settings.lastClient, lastCharacter: current },
          },
      );
  };

  const play = () => startRows(rowsNow(), { remember: true });
  keysRef.current = {
    play: () => !launching && !busy && !signingIn && current && play(),
    settings: () => setSettingsOpen(true),
  };

  const checkUpdate = () =>
    api
      .checkUpdate()
      .then((r) => {
        if (!r.newer) return setNotice({ kind: 'ok', text: `You have the latest version (${state.version}).` });
        setUpdate(r);
        setSettingsOpen(false);
        setUpdateView({ release: r, mode: 'update' });
      })
      .catch(fail);
  const installUpdate = () => {
    setUpdateProgress({ phase: 'downloading', received: 0, total: 0, message: '' });
    api.installUpdate().catch((e) => setUpdateProgress({ phase: 'error', received: 0, total: 0, message: errorText(e) }));
  };

  const useJar = (on: boolean) => {
    if (client === 'rs3') return;
    persist({ ...state.settings, [client]: { ...state.settings[client], useJar: on } });
  };

  const useDebug = (on: boolean) => {
    if (client === 'rs3') return;
    persist({ ...state.settings, [client]: { ...state.settings[client], debug: on } });
  };

  const groups = state.settings.groups;
  const saveGroup = (name: string) => {
    const group: Group = { name, game, rows: rowsNow() };
    // The same name on the same tab replaces the old group.
    const i = groups.findIndex((g) => g.game === game && g.name.toLowerCase() === name.toLowerCase());
    persist({ ...state.settings, groups: i < 0 ? [...groups, group] : groups.map((g, j) => (j === i ? group : g)) });
    setNotice({ kind: 'ok', text: `Saved “${name}”. It's in the tray menu too.` });
  };
  const loadGroup = (n: number) => {
    const g = groups[n];
    if (!g?.rows.length) return;
    const [first, ...rest] = g.rows;
    setGame(g.game);
    setPick(first!.pick);
    if (g.game === 'osrs') {
      const c = first!.client === 'hdos' ? 'hdos' : 'runelite';
      setOsrsClient(c);
      if (hasJar(c) && state.settings[c].useJar !== first!.jar) persist({ ...state.settings, [c]: { ...state.settings[c], useJar: first!.jar } });
    }
    setExtras(rest);
  };
  const playGroup = (n: number) => {
    const g = groups[n];
    if (!g || launching) return;
    const rows = g.rows.filter((x) => validPick(x.pick, state.accounts)).map((x) => ({ ...x, jar: x.jar && hasJar(x.client) }));
    if (!rows.length) return setNotice({ kind: 'warn', text: `None of the characters in “${g.name}” is here any more.` });
    startRows(rows);
  };
  playGroupRef.current = playGroup;
  const deleteGroup = (n: number) => persist({ ...state.settings, groups: groups.filter((_, j) => j !== n) });

  const restart = (pid: number) => {
    setRestarting((r) => [...r, pid]);
    api
      .restartInstance(pid)
      .then((i) => setNotice({ kind: 'ok', text: `${i.character} restarted.` }))
      .catch(fail)
      .finally(() => setRestarting((r) => r.filter((x) => x !== pid)));
  };

  /** Build the plugin projects; only if that works, start the rows (restarting characters that run). */
  const buildAndPlay = async () => {
    setBuild({ lines: [], phase: 'running', message: '' });
    try {
      const jars = await api.buildPlugins();
      setBuild((old) => old && { ...old, phase: 'ok', message: `Copied ${jars.join(', ')}.` });
    } catch (e) {
      setBuild((old) => old && { ...old, phase: 'failed', message: errorText(e) });
      return;
    }
    setBuild(null);
    await startRows(rowsNow(), { remember: true, restart: true });
  };

  const updateClient = (c: ClientId) =>
    api
      .updateClients(c, true)
      .then((clients) => setState((old) => old && { ...old, clients }))
      .catch(fail);

  const live = (k: Pick) => {
    const parts = splitPick(k);
    return state.instances.filter((i) => parts && i.accountId === parts[0] && i.characterId === parts[1]).map((i) => i.startedAt);
  };

  // Closes at once; the save runs behind it.
  const saveSettings = (settings: Settings) => {
    setLook(null);
    setSettingsOpen(false);
    persist(settings);
  };
  const closeSettings = () => {
    setLook(null);
    setSettingsOpen(false);
  };
  const fold = (side: 'hideAccounts' | 'hideInstances') => persist({ ...state.settings, [side]: !state.settings[side] });

  const openFolder = (which: Folder) => api.openFolder(which).catch(fail);

  return (
    <div
      className={['app', shown.density === 'compact' && 'compact', state.settings.hideAccounts && 'no-left', state.settings.hideInstances && 'no-right']
        .filter(Boolean)
        .join(' ')}
    >
      <header className="topbar">
        <button
          className="icon-btn"
          title={state.settings.hideAccounts ? 'Show accounts' : 'Hide accounts'}
          aria-pressed={!state.settings.hideAccounts}
          onClick={() => fold('hideAccounts')}
        >
          {state.settings.hideAccounts ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
        </button>
        <div className="brand">
          <img src={logo} alt="" />
          <span className="void">VOID</span>
          <span className="name">Lodestone</span>
        </div>
        <nav className="tabs" role="tablist" aria-label="Game" style={{ marginLeft: 18 }}>
          {(
            [
              ['osrs', 'Old School'],
              ['rs3', 'RuneScape 3'],
            ] as [Game, string][]
          ).map(([id, label]) => (
            <button key={id} role="tab" aria-selected={game === id} className={`tab ${game === id ? 'active' : ''}`} onClick={() => setGame(id)}>
              <span className="serif">{label}</span>
            </button>
          ))}
        </nav>
        <span className="spacer" />
        {state.secretBackend === 'file' && (
          <span className="badge warn" title="No Secret Service keyring was found, so sessions are kept in a private file.">
            no keyring
          </span>
        )}
        <button
          className="icon-btn"
          title={state.settings.hideInstances ? 'Show running games' : 'Hide running games'}
          aria-pressed={!state.settings.hideInstances}
          onClick={() => fold('hideInstances')}
        >
          {state.settings.hideInstances ? <PanelRightOpen size={16} /> : <PanelRightClose size={16} />}
          {state.settings.hideInstances && state.instances.length > 0 && <span className="fold-count">{state.instances.length}</span>}
        </button>
        {update && (
          <button className="update-pill" title={`Lodestone ${update.version} is out`} onClick={() => setUpdateView({ release: update, mode: 'update' })}>
            <ArrowUpCircle size={14} /> {updateProgress?.phase === 'done' ? 'Restart to update' : `Update ${update.version}`}
          </button>
        )}
        <button className="icon-btn" title="Settings (Ctrl+,)" onClick={() => setSettingsOpen(true)}>
          <Cog size={16} />
        </button>
      </header>

      <div className="body">
        {!state.settings.hideAccounts && (
          <Accounts
            accounts={state.accounts}
            instances={state.instances}
            pick={current}
            busy={busy || signingIn}
            onPick={setPick}
            onAdd={() => setChoosing({})}
            onReauth={(id) => setChoosing({ id })}
            onRemove={(id) => withBusy(async () => setState(await api.removeAccount(id)))}
            onRename={(id, label) =>
              withBusy(async () => {
                await api.renameAccount(id, label);
                setState(await api.getState());
              })
            }
            onMove={(id, to) =>
              withBusy(async () => {
                await api.moveAccount(id, to);
                setState(await api.getState());
              })
            }
            onRefresh={() => withBusy(async () => setState(await api.refreshAccounts()))}
          />
        )}
        <Stage
          game={game}
          client={client}
          accounts={state.accounts}
          clients={state.clients}
          progress={progress}
          pick={current}
          launching={launching}
          busy={busy || signingIn}
          onClient={(c) => game === 'osrs' && setOsrsClient(c)}
          onPick={setPick}
          onPlay={play}
          onUpdate={updateClient}
          onAdd={() => setChoosing({})}
          jarPath={client === 'rs3' ? '' : state.settings[client].jarPath.trim()}
          useJar={client !== 'rs3' && state.settings[client].useJar}
          onUseJar={useJar}
          debug={client !== 'rs3' && state.settings[client].debug}
          onDebug={useDebug}
          jars={{ runelite: state.settings.runelite.jarPath.trim(), hdos: state.settings.hdos.jarPath.trim() }}
          extras={extraRows}
          onExtras={setExtras}
          groups={groups.flatMap((group, n) => (group.game === game ? [{ n, group }] : []))}
          onLoadGroup={loadGroup}
          onPlayGroup={playGroup}
          onDeleteGroup={deleteGroup}
          onSaveGroup={saveGroup}
          onBuild={game === 'osrs' && state.settings.pluginProjects.trim() ? buildAndPlay : undefined}
          building={build?.phase === 'running'}
          played={current ? { entry: playtime[current], live: live(current) } : null}
          background={background?.url}
          backgroundDim={shown.backgroundDim}
          backgroundBlur={background?.cssBlur}
          backgroundPosition={shown.backgroundPosition}
          hideHero={!shown.showHero}
        />
        {!state.settings.hideInstances && (
          <Instances
            instances={state.instances}
            onStop={(pid) => api.stopInstance(pid).catch(fail)}
            onStopAll={() => api.stopAll().catch(fail)}
            onRestart={restart}
            restarting={restarting}
            onLogs={() => openFolder('logs')}
            onLog={(inst) => setLogOf({ inst, atError: false })}
          />
        )}
      </div>

      {notice && (
        <div className={`notice toast ${notice.kind}`} role={notice.kind === 'error' ? 'alert' : 'status'}>
          {notice.kind === 'ok' ? (
            <CheckCircle2 size={15} color="var(--success)" />
          ) : (
            <AlertTriangle size={15} color={notice.kind === 'warn' ? 'var(--warning)' : 'var(--danger)'} />
          )}
          <span className="grow selectable">
            {notice.text}
            {notice.crashed && (
              <span className="notice-actions">
                <button className="btn small" onClick={() => setLogOf({ inst: notice.crashed!, atError: true })}>
                  Open log
                </button>
                <button
                  className="btn small"
                  onClick={() => {
                    restart(notice.crashed!.pid);
                    setNotice(null);
                  }}
                >
                  Restart
                </button>
              </span>
            )}
          </span>
          <button className="icon-btn" style={{ width: 22, height: 22 }} title="Dismiss" onClick={() => setNotice(null)}>
            <X size={13} />
          </button>
        </div>
      )}

      {choosing && !signingIn && <LoginChoice again={!!choosing.id} onPick={(mode) => signIn(mode, choosing.id)} onCancel={() => setChoosing(null)} />}
      {signingIn && <LoginOverlay status={login} mode={loginMode} onCancel={() => api.cancelLogin().catch(fail)} />}

      {logOf && (
        <LogViewer
          key={logOf.inst.pid}
          instance={logOf.inst}
          atError={logOf.atError}
          packages={state.pluginPackages}
          onClose={() => setLogOf(null)}
          onOpenFolder={() => openFolder('logs')}
        />
      )}

      {build && <BuildModal build={build} onCancel={() => api.cancelBuild().catch(fail)} onClose={() => setBuild(null)} />}

      {settingsOpen && (
        <SettingsModal state={state} onPreview={setLook} onSave={saveSettings} onClose={closeSettings} onOpenFolder={openFolder} onCheckUpdate={checkUpdate} />
      )}

      {updateView && (
        <UpdateModal
          release={updateView.release}
          mode={updateView.mode}
          current={state.version}
          progress={updateView.mode === 'update' ? updateProgress : null}
          gamesRunning={state.instances.length > 0}
          onInstall={installUpdate}
          onRestart={() => api.restartApp().catch(fail)}
          onOpenPage={() => openFolder('releases')}
          onClose={() => setUpdateView(null)}
        />
      )}
    </div>
  );
}
