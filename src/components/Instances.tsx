import { useEffect, useState } from 'react';
import { LayoutGrid, RotateCw, ScrollText, Square, Undo2 } from 'lucide-react';
import { CLIENTS, uptime } from '../format';
import type { Instance, Monitor } from '../types';

interface Props {
  instances: Instance[];
  onStop: (pid: number) => void;
  onStopAll: () => void;
  /** Start the same character again with the same client and jar. */
  onRestart: (pid: number) => void;
  /** Games being restarted right now. */
  restarting?: number[];
  onLogs: () => void;
  onLog: (i: Instance) => void;
  /** Tile the game windows over every monitor, or the one given. */
  onTile?: (monitor: number | null) => void;
  /** Put the windows back where each character had them. */
  onRestoreWindows?: () => void;
  monitors?: Monitor[];
}

export default function Instances({ instances, onStop, onStopAll, onRestart, restarting = [], onLogs, onLog, onTile, onRestoreWindows, monitors = [] }: Props) {
  const [screen, setScreen] = useState('all');
  const [now, setNow] = useState(() => Date.now() / 1000);
  useEffect(() => {
    if (!instances.length) return;
    const t = setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => clearInterval(t);
  }, [instances.length]);

  return (
    <aside className="side right" aria-label="Running games">
      <div className="side-head">
        <span>Running</span>
        <span className="badge">{instances.length}</span>
        <span className="spacer" />
        {instances.length > 1 && (
          <button className="btn ghost small" title="Stop every running game" onClick={onStopAll}>
            Stop all
          </button>
        )}
        <button className="btn ghost small" title="Open the logs folder" onClick={onLogs}>
          Logs
        </button>
      </div>
      {onTile && instances.length > 0 && (
        <div className="window-tools" role="group" aria-label="Game windows">
          <button className="btn small" title="Lay the game windows out side by side. Only moves windows." onClick={() => onTile(screen === 'all' ? null : Number(screen))}>
            <LayoutGrid size={12} /> Tile
          </button>
          {monitors.length > 1 && (
            <select className="select small" aria-label="Tile over" value={screen} onChange={(e) => setScreen(e.target.value)}>
              <option value="all">All monitors</option>
              {monitors.map((m, n) => (
                <option key={n} value={n}>
                  Monitor {n + 1}
                  {m.primary ? ' (main)' : ''} · {m.width}×{m.height}
                </option>
              ))}
            </select>
          )}
          {onRestoreWindows && (
            <button className="btn small ghost" title="Put each window back where that character had it" onClick={onRestoreWindows}>
              <Undo2 size={12} /> Restore
            </button>
          )}
        </div>
      )}
      <div className="side-scroll">
        {!instances.length && <div className="empty">Nothing running. Games you launch show up here.</div>}
        {instances.map((i) => (
          <div className="instance" key={i.pid}>
            <div className="instance-top">
              <span className="client-mark">{CLIENTS[i.client].mark}</span>
              <span className="who">
                <b>{i.character}</b>
                <span>
                  {CLIENTS[i.client].name} · {i.accountLabel}
                </span>
              </span>
              <button className="icon-btn" title="Live log" onClick={() => onLog(i)}>
                <ScrollText size={13} />
              </button>
              <button className="icon-btn" title="Restart: same character, client and jar" disabled={restarting.includes(i.pid)} onClick={() => onRestart(i.pid)}>
                <RotateCw size={13} className={restarting.includes(i.pid) ? 'spin' : undefined} />
              </button>
              <button className="icon-btn danger" title="Stop this game" onClick={() => onStop(i.pid)}>
                <Square size={13} />
              </button>
            </div>
            <div className="instance-foot">
              <span className="mono">{uptime(i.startedAt, now)}</span>
              {i.customJar && (
                <span className="badge warn" title="Started with the custom jar">
                  custom jar
                </span>
              )}
              {i.debugPort != null && (
                <span className="badge" title={`Attach a debugger (remote JVM, socket) to 127.0.0.1:${i.debugPort}`}>
                  debug :{i.debugPort}
                </span>
              )}
              <span className="spacer" />
              <span className="mono">pid {i.pid}</span>
            </div>
            {i.rebuilt && (
              <button className="rebuilt" disabled={restarting.includes(i.pid)} onClick={() => onRestart(i.pid)} title="The jar or a sideloaded plugin changed since this game started">
                <RotateCw size={12} /> Rebuilt, restart?
              </button>
            )}
          </div>
        ))}
      </div>
    </aside>
  );
}
