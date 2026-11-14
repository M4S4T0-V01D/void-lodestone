import { useEffect, useState } from 'react';
import { Archive, ArchiveRestore, Check, FolderOpen, Share2, Undo2, X } from 'lucide-react';
import { api, errorText, type Folder } from '../api';
import { ago, bytes, CLIENTS } from '../format';
import type { AppState, Backup, ClientStatus, Profile, Settings } from '../types';

export type Notify = (kind: 'ok' | 'error' | 'warn', text: string) => void;

/** Copy one RuneLite profile's settings over others. */
export function ShareProfiles({ notify }: { notify: Notify }) {
  const [profiles, setProfiles] = useState<Profile[] | null>(null);
  const [error, setError] = useState('');
  const [from, setFrom] = useState('');
  const [to, setTo] = useState<string[]>([]);
  const [working, setWorking] = useState(false);

  const load = () =>
    api
      .runeliteProfiles()
      .then((list) => {
        setProfiles(list);
        setFrom((f) => f || list.find((x) => x.active)?.name || list[0]?.name || '');
      })
      .catch((e) => setError(errorText(e)));
  useEffect(() => {
    load();
  }, []);

  const share = () => {
    setWorking(true);
    api
      .shareProfile(from, to)
      .then((n) => {
        notify('ok', `Copied “${from}” over ${n} profile${n === 1 ? '' : 's'}. The old files were kept as .bak.`);
        setTo([]);
      })
      .catch((e) => notify('error', errorText(e)))
      .finally(() => setWorking(false));
  };

  if (error) return <span className="hint">{error}</span>;
  if (!profiles) return <span className="hint">Reading RuneLite's profiles…</span>;
  if (profiles.length < 2)
    return <span className="hint">RuneLite has {profiles.length ? 'one profile' : 'no profiles'} yet. They appear once characters have played.</span>;
  const synced = profiles.some((x) => to.includes(x.name) && x.sync);
  return (
    <div className="share">
      <div className="field">
        <label htmlFor="share-from">Copy the settings of</label>
        <select
          id="share-from"
          className="select"
          value={from}
          onChange={(e) => {
            setFrom(e.target.value);
            setTo((t) => t.filter((x) => x !== e.target.value));
          }}
        >
          {profiles.map((x) => (
            <option key={x.id} value={x.name}>
              {x.name}
              {x.active ? ' (active)' : ''}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <span className="label">Over</span>
        <div className="share-to">
          {profiles
            .filter((x) => x.name !== from)
            .map((x) => (
              <label className="check" key={x.id}>
                <input
                  type="checkbox"
                  checked={to.includes(x.name)}
                  onChange={(e) => setTo((t) => (e.target.checked ? [...t, x.name] : t.filter((y) => y !== x.name)))}
                />
                <span>
                  {x.name}
                  {x.sync && <span className="badge">synced</span>}
                </span>
              </label>
            ))}
        </div>
        <span className="hint">
          {synced
            ? 'Synced profiles live in your RuneLite account; RuneLite may put its cloud copy back over this one.'
            : 'Plugins, layouts and key bindings all come along. Close RuneLite first.'}
        </span>
      </div>
      <button className="btn" style={{ alignSelf: 'flex-start' }} disabled={!from || !to.length || working} onClick={share}>
        <Share2 size={14} /> {working ? 'Copying…' : `Copy to ${to.length || 'the ticked'} profile${to.length === 1 ? '' : 's'}`}
      </button>
    </div>
  );
}

/** The selected client's version, and going back to the one before. */
export function Rollback({ status, onClients, notify }: { status?: ClientStatus; onClients: (c: ClientStatus[]) => void; notify: Notify }) {
  const [working, setWorking] = useState(false);
  if (!status?.installed) return null;
  const name = CLIENTS[status.client].name;
  const what = status.client === 'rs3' ? 'client' : 'launcher';
  const run = (f: () => Promise<ClientStatus[]>, done: string) => {
    setWorking(true);
    f()
      .then((c) => {
        onClients(c);
        notify('ok', done);
      })
      .catch((e) => notify('error', errorText(e)))
      .finally(() => setWorking(false));
  };
  return (
    <div className="rollback">
      <span className="dim">
        {status.version ? `Version ${status.version}` : 'Installed'}
        {status.pinned && ' · updates paused'}
      </span>
      {status.pinned ? (
        <button className="btn small" disabled={working} onClick={() => run(() => api.resumeUpdates(status.client), `${name} updates again.`)}>
          <Check size={12} /> Resume updates
        </button>
      ) : (
        status.canRollBack && (
          <button
            className="btn small"
            disabled={working}
            title={`Put the ${what} you had before back, and stop updating it until you resume`}
            onClick={() =>
              run(() => api.rollbackClient(status.client), `${name} is back on ${status.previousVersion || 'the version before'}. Updates are paused.`)
            }
          >
            <Undo2 size={12} /> Roll back to {status.previousVersion || 'the previous version'}
          </button>
        )
      )}
      {status.client !== 'rs3' && (status.pinned || status.canRollBack) && (
        <span className="hint">
          This rolls back {name}'s launcher only; it fetches the game client itself
          {status.client === 'runelite' ? ', so the client and plugins update as usual' : ''}.
        </span>
      )}
    </div>
  );
}

/** Backups of Lodestone's settings and RuneLite's profiles, and bringing RuneLite's settings over from Bolt. */
export function BackupPanel(p: {
  state: AppState;
  notify: Notify;
  onOpenFolder: (which: Folder) => void;
  /** A restore replaced the settings, so the Settings window closes with the new state. */
  onRestored: (s: AppState) => void;
}) {
  const [list, setList] = useState<Backup[]>([]);
  const [working, setWorking] = useState(false);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [parts, setParts] = useState({ lodestone: true, runelite: true });
  const [bolt, setBolt] = useState<string[]>([]);
  const [folder, setFolder] = useState('');

  const load = () =>
    api
      .backups()
      .then(setList)
      .catch(() => {});
  useEffect(() => {
    load();
    api
      .boltFolders()
      .then((f) => {
        setBolt(f);
        setFolder((old) => old || f[0] || '');
      })
      .catch(() => {});
  }, []);

  const busy = async (f: () => Promise<void>) => {
    setWorking(true);
    try {
      await f();
    } catch (e) {
      p.notify('error', errorText(e));
    } finally {
      setWorking(false);
    }
  };
  const make = () =>
    busy(async () => {
      const name = await api.makeBackup();
      p.notify('ok', `Saved ${name} in your Documents folder.`);
      await load();
    });
  const restore = (name: string) =>
    busy(async () => {
      const s = await api.restoreBackup(name, parts.lodestone, parts.runelite);
      p.notify('ok', `Restored ${name}. What it replaced was kept beside it as “before-restore”.`);
      p.onRestored(s);
    });
  const importBolt = () =>
    busy(async () => {
      const n = await api.importBolt(folder);
      p.notify('ok', `Copied ${n} RuneLite file${n === 1 ? '' : 's'} from Bolt. Your old ones were kept as “before-restore”.`);
    });
  const running = p.state.instances.some((i) => i.client === 'runelite');

  return (
    <>
      <div className="group">
        <h3>
          <Archive size={14} /> Backups
        </h3>
        <span className="hint">
          One file with Lodestone's settings, groups, character notes and play time, plus RuneLite's settings and profiles. Never your sign-in sessions: after
          restoring on another computer, sign in again.
        </span>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <button className="btn primary" disabled={working} onClick={make}>
            <Archive size={14} /> Back up now
          </button>
          <button className="btn" onClick={() => p.onOpenFolder('backups')}>
            <FolderOpen size={14} /> Backups folder
          </button>
        </div>
        {list.length > 0 && (
          <ul className="backups">
            {list.map((b) => (
              <li key={b.name}>
                <span className="grow">
                  <b>{b.name}</b>
                  <span className="dim">
                    {' '}
                    · {ago(b.created)} · {bytes(b.size)}
                  </span>
                </span>
                {restoring === b.name ? (
                  <span className="restore-pick">
                    <label className="check">
                      <input type="checkbox" checked={parts.lodestone} onChange={(e) => setParts((o) => ({ ...o, lodestone: e.target.checked }))} />
                      <span>Lodestone</span>
                    </label>
                    <label className="check">
                      <input type="checkbox" checked={parts.runelite} onChange={(e) => setParts((o) => ({ ...o, runelite: e.target.checked }))} />
                      <span>RuneLite</span>
                    </label>
                    <button
                      className="btn small primary"
                      disabled={working || (!parts.lodestone && !parts.runelite) || (parts.runelite && running)}
                      title={parts.runelite && running ? 'Close RuneLite first' : undefined}
                      onClick={() => restore(b.name)}
                    >
                      Restore
                    </button>
                    <button className="icon-btn" title="Cancel" onClick={() => setRestoring(null)}>
                      <X size={13} />
                    </button>
                  </span>
                ) : (
                  <button className="btn small" onClick={() => setRestoring(b.name)}>
                    <ArchiveRestore size={12} /> Restore…
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
        <span className="hint">
          Restoring adds the backup's accounts to the ones here and takes its settings. To restore a backup from elsewhere, put it in the backups folder.
        </span>
      </div>

      <div className="group">
        <h3>Coming from Bolt</h3>
        <span className="hint">
          Copies RuneLite's settings and profiles (plugins, layouts, key bindings) from Bolt. Bolt's saved sign-in is never read: add your Jagex accounts here
          with “Add account”.
        </span>
        {p.state.flatpak && !bolt.length && (
          <span className="hint">
            The Flatpak can't see Bolt's own folder. Copy <code>~/.var/app/com.adamcake.Bolt/data/bolt-launcher</code> into your Documents folder first, then
            pick it below.
          </span>
        )}
        <div className="field">
          <label htmlFor="bolt-folder">Bolt folder</label>
          <input
            id="bolt-folder"
            className="input mono"
            list="bolt-found"
            placeholder="~/Documents/bolt-launcher"
            value={folder}
            onChange={(e) => setFolder(e.target.value)}
          />
          <datalist id="bolt-found">
            {bolt.map((f) => (
              <option key={f} value={f} />
            ))}
          </datalist>
          {bolt.length > 0 && <span className="hint">Found {bolt.length === 1 ? 'one' : bolt.length}: pick from the list or type another folder.</span>}
        </div>
        <button
          className="btn"
          style={{ alignSelf: 'flex-start' }}
          disabled={working || !folder.trim() || running}
          title={running ? 'Close RuneLite first' : undefined}
          onClick={importBolt}
        >
          <ArchiveRestore size={14} /> Copy RuneLite settings from Bolt
        </button>
      </div>
    </>
  );
}

/** The window and profile switches on the Characters tab. */
export function CharacterSwitches({ s, set }: { s: Settings; set: <K extends keyof Settings>(k: K, v: Settings[K]) => void }) {
  return (
    <>
      <label className="check">
        <input type="checkbox" checked={s.runeliteProfiles} onChange={(e) => set('runeliteProfiles', e.target.checked)} />
        <span>
          A RuneLite profile per character
          <span className="hint dim">
            Each character starts RuneLite with a profile named after it, made from your active profile the first time. A character's own choice in its notes
            wins.
          </span>
        </span>
      </label>
      <label className="check">
        <input type="checkbox" checked={s.windowTitles} onChange={(e) => set('windowTitles', e.target.checked)} />
        <span>
          Name the game windows
          <span className="hint dim">Puts the character's name in the window title, for the taskbar and window switcher.</span>
        </span>
      </label>
      <label className="check">
        <input type="checkbox" checked={s.rememberWindows} onChange={(e) => set('rememberWindows', e.target.checked)} />
        <span>
          Remember where each character's window was
          <span className="hint dim">
            And put it back there next time. Needs X11 or XWayland. Lodestone only moves windows: it never sends keys or clicks to a game.
          </span>
        </span>
      </label>
    </>
  );
}
