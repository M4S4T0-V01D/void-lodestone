import { useEffect, useState } from 'react';
import { Square } from 'lucide-react';
import { CLIENTS, uptime } from '../format';
import type { Instance } from '../types';

interface Props {
  instances: Instance[];
  onStop: (pid: number) => void;
  onLogs: () => void;
}

export default function Instances({ instances, onStop, onLogs }: Props) {
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
        <button className="btn ghost small" onClick={onLogs}>
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
              <button className="icon-btn danger" title="Stop this game" onClick={() => onStop(i.pid)}>
                <Square size={13} />
              </button>
            </div>
            <div className="instance-foot">
              <span className="mono">{uptime(i.startedAt, now)}</span>
              <span className="mono">pid {i.pid}</span>
            </div>
          </div>
        ))}
      </div>
    </aside>
  );
}
