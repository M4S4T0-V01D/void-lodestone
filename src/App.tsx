import { useCallback, useEffect, useRef, useState } from 'react';
import { AlertTriangle, CheckCircle2, Settings as Cog, X } from 'lucide-react';
import { api, errorText, events, type Folder } from './api';
import logo from './assets/logo.svg';
import Accounts from './components/Accounts';
import BuildModal, { type Build } from './components/BuildModal';
import Instances from './components/Instances';
import LoginChoice from './components/LoginChoice';
import LoginOverlay from './components/LoginOverlay';
import LogViewer from './components/LogViewer';
import SettingsModal from './components/SettingsModal';
import Stage from './components/Stage';
import { characterName, CLIENTS, loadRows, resolvePick, splitPick, validPick, type Row } from './format';
import { applyTheme, findTheme } from './themes';
import type { AppState, ClientId, Game, Group, Instance, LoginMode, LoginStatus, Pick, Playtime, Progress, Settings } from './types';

/** `crashed`: the game that crashed, for the Open log and Restart buttons. */
type Notice = { kind: 'ok' | 'error' | 'warn'; text: string; crashed?: Instance };

/** How often the rotating background changes. */
const ROTATE_MS = 5 * 60 * 1000;

function crashText(i: Instance): string {
  const why = i.crashLine ? `: ${i.crashLine}` : i.exitCode ? ` (exit code ${i.exitCode}).` : ' within a minute of starting.';
  return `${i.character} (${CLIENTS[i.client].name}) crashed${why}`;
}

function useTheme(theme: string | undefined, reduceMotion: boolean | undefined) {
  useEffect(() => {
    if (!theme) return;
    applyTheme(findTheme(theme));
    localStorage.setItem('lodestone.theme', theme);
  }, [theme]);
  useEffect(() => {
    document.documentElement.classList.toggle('reduce-motion', !!reduceMotion);
  }, [reduceMotion]);
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
  const [background, setBackground] = useState('');
  const [logOf, setLogOf] = useState<{ inst: Instance; atError: boolean } | null>(null);
  const [playtime, setPlaytime] = useState<Record<string, Playtime>>({});
  const [restarting, setRestarting] = useState<number[]>([]);
  const [build, setBuild] = useState<Build | null>(null);
  const [shots, setShots] = useState<string[]>([]);
  const [shot, setShot] = useState(0);
  /** Starts a saved group; the tray's menu calls it. Set on every render so it sees the current state. */
  const playGroupRef = useRef<(n: number) => void>(() => {});

  useTheme(state?.settings.theme, state?.settings.reduceMotion);

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

  // The rotating background: RuneLite's screenshots, on the Old School tab.
  const rotate = !!state?.settings.backgroundRotate && game === 'osrs';
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
    const t = setInterval(() => setShot((n) => n + 1 + Math.floor(Math.random() * 7)), ROTATE_MS);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [rotate]);

  const rs3Bg = state?.settings.backgroundRs3.trim() ?? '';
  const bgPath =
    rotate && shots.length ? shots[shot % shots.length]! : game === 'rs3' && rs3Bg ? rs3Bg : (state?.settings.background.trim() ?? '');
  useEffect(() => {
    let alive = true;
    if (!bgPath) {
      setBackground('');
      return;
    }
    api
      .backgroundImage(bgPath)
      .then((url) => alive && setBackground(url))
      .catch((e) => {
        if (!alive) return;
        // A screenshot that went away: try another next time.
        if (rotate && shots.length) return;
        setBackground('');
        setNotice({ kind: 'warn', text: `Background picture: ${errorText(e)}` });
      });
    return () => {
      alive = false;
    };
  }, [bgPath, rotate, shots.length]);

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
  if (!state) return <div className="app" />;

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

  const saveSettings = (settings: Settings) =>
    api
      .saveSettings(settings)
      .then(() => {
        setState((old) => old && { ...old, settings });
        setSettingsOpen(false);
      })
      .catch(fail);

  const openFolder = (which: Folder) => api.openFolder(which).catch(fail);

  return (
    <div className="app">
      <header className="topbar">
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
        <button className="icon-btn" title="Settings" onClick={() => setSettingsOpen(true)}>
          <Cog size={16} />
        </button>
      </header>

      <div className="body">
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
          background={background}
          backgroundDim={state.settings.backgroundDim}
          backgroundBlur={state.settings.backgroundBlur}
        />
        <Instances
          instances={state.instances}
          onStop={(pid) => api.stopInstance(pid).catch(fail)}
          onStopAll={() => api.stopAll().catch(fail)}
          onRestart={restart}
          restarting={restarting}
          onLogs={() => openFolder('logs')}
          onLog={(inst) => setLogOf({ inst, atError: false })}
        />
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
        <SettingsModal
          state={state}
          onPreviewTheme={(id) => applyTheme(findTheme(id))}
          onSave={saveSettings}
          onClose={() => setSettingsOpen(false)}
          onOpenFolder={openFolder}
        />
      )}
    </div>
  );
}
