import { useEffect, useState } from 'react';
import { RotateCw, ScrollText, Square } from 'lucide-react';
import { CLIENTS, uptime } from '../format';
import type { Instance } from '../types';

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
}

export default function Instances({ instances, onStop, onStopAll, onRestart, restarting = [], onLogs, onLog }: Props) {
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
