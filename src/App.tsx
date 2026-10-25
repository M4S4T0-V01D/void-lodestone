import { useCallback, useEffect, useState } from 'react';
import { AlertTriangle, CheckCircle2, Settings as Cog, X } from 'lucide-react';
import { api, errorText, events, type Folder } from './api';
import logo from './assets/logo.svg';
import Accounts from './components/Accounts';
import Instances from './components/Instances';
import LoginOverlay from './components/LoginOverlay';
import SettingsModal from './components/SettingsModal';
import Stage from './components/Stage';
import { CLIENTS, resolvePick, splitPick } from './format';
import { applyTheme, findTheme } from './themes';
import type { AppState, ClientId, Game, LoginStatus, Pick, Progress, Settings } from './types';

type Notice = { kind: 'ok' | 'error' | 'warn'; text: string };

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
  const [notice, setNotice] = useState<Notice | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);

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
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    const subs = [
      events.progress((p) => {
        setProgress((old) => ({ ...old, [p.client]: p }));
        if (p.phase === 'ready')
          api
            .getState()
            .then((s) => setState((old) => (old ? { ...old, clients: s.clients } : s)))
            .catch(() => {});
      }),
      events.instances((instances) => setState((old) => (old ? { ...old, instances } : old))),
      events.login(setLogin),
    ];
    return () => subs.forEach((p) => p.then((off) => off()));
  }, []);

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

  const signIn = async (id?: string) => {
    const before = new Set(state.accounts.map((a) => a.id));
    setSigningIn(true);
    setLogin({ phase: 'waiting', message: '' });
    try {
      const s = id ? await api.reauthAccount(id) : await api.addAccount();
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

  const play = async () => {
    const parts = splitPick(current);
    if (!parts) return;
    setLaunching(true);
    setNotice(null);
    try {
      const inst = await api.launch(client, parts[0], parts[1]);
      setNotice({ kind: 'ok', text: `${CLIENTS[client].name} started for ${inst.character}.` });
      setState(
        (old) =>
          old && {
            ...old,
            settings: { ...old.settings, lastGame: game, lastClient: game === 'osrs' ? client : old.settings.lastClient, lastCharacter: current },
          },
      );
    } catch (e) {
      fail(e);
      // A failed launch may have found an expired session.
      api
        .getState()
        .then(setState)
        .catch(() => {});
    } finally {
      setLaunching(false);
    }
  };

  const updateClient = (c: ClientId) =>
    api
      .updateClients(c, true)
      .then((clients) => setState((old) => old && { ...old, clients }))
      .catch(fail);

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
          onAdd={() => signIn()}
          onReauth={(id) => signIn(id)}
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
          onAdd={() => signIn()}
        />
        <Instances instances={state.instances} onStop={(pid) => api.stopInstance(pid).catch(fail)} onLogs={() => openFolder('logs')} />
      </div>

      {notice && (
        <div className={`notice toast ${notice.kind}`} role={notice.kind === 'error' ? 'alert' : 'status'}>
          {notice.kind === 'ok' ? <CheckCircle2 size={15} color="var(--success)" /> : <AlertTriangle size={15} color="var(--danger)" />}
          <span className="grow selectable">{notice.text}</span>
          <button className="icon-btn" style={{ width: 22, height: 22 }} title="Dismiss" onClick={() => setNotice(null)}>
            <X size={13} />
          </button>
        </div>
      )}

      {signingIn && <LoginOverlay status={login} onCancel={() => api.cancelLogin().catch(fail)} />}

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
