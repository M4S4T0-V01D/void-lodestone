import { useEffect, useState } from 'react';
import { ExternalLink, RefreshCw, X } from 'lucide-react';
import { api, errorText } from '../api';
import { ago } from '../format';
import type { Game, NewsItem, ServerStatus } from '../types';

interface Props {
  game: Game;
  onClose: () => void;
}

/** "Wed, 11 Nov 2026 11:00:00 GMT" → "11 Nov 2026". */
export function newsDate(s: string): string {
  const d = new Date(s);
  return Number.isNaN(d.getTime()) ? s : d.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
}

function count(n: number | null): string {
  return n == null ? '–' : n.toLocaleString();
}

/** Jagex's latest news for either game, and whether the servers and sign-in are up. Fetched only while open. */
export default function NewsModal(p: Props) {
  const [game, setGame] = useState<Game>(p.game);
  const [items, setItems] = useState<Partial<Record<Game, NewsItem[]>>>({});
  const [error, setError] = useState('');
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [loading, setLoading] = useState(0);

  const load = (g: Game) => {
    setError('');
    setLoading((n) => n + 1);
    api
      .news(g)
      .then((list) => setItems((o) => ({ ...o, [g]: list })))
      .catch((e) => setError(errorText(e)))
      .finally(() => setLoading((n) => n - 1));
  };
  const check = () => {
    setLoading((n) => n + 1);
    api
      .serverStatus()
      .then(setStatus)
      .catch(() => {})
      .finally(() => setLoading((n) => n - 1));
  };

  useEffect(check, []);
  useEffect(() => {
    if (!items[game]) load(game);
  }, [game]); // eslint-disable-line react-hooks/exhaustive-deps

  const list = items[game];
  const worldsUp = status?.osrsWorlds != null && status.osrsOnline != null;
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && p.onClose()}>
      <div className="modal news-modal" role="dialog" aria-modal="true" aria-label="News" onKeyDown={(e) => e.key === 'Escape' && p.onClose()}>
        <div className="modal-head">
          <h2>News &amp; status</h2>
          <button
            className="icon-btn"
            title="Refresh"
            disabled={loading > 0}
            onClick={() => {
              check();
              load(game);
            }}
          >
            <RefreshCw size={15} className={loading > 0 ? 'spin' : undefined} />
          </button>
          <button className="icon-btn" title="Close" onClick={p.onClose}>
            <X size={16} />
          </button>
        </div>
        <div className="server-status" role="status">
          <span className={`badge ${!status ? '' : status.signIn ? 'ok' : 'bad'}`}>Sign-in {!status ? '…' : status.signIn ? 'up' : 'not answering'}</span>
          <span className={`badge ${!status ? '' : worldsUp && status.osrsOnline! > 0 ? 'ok' : 'bad'}`}>
            Old School worlds {worldsUp ? `${status!.osrsOnline}/${status!.osrsWorlds}` : status ? 'unknown' : '…'}
          </span>
          <span className="dim">
            {count(status?.osrsPlayers ?? null)} on Old School · {count(status?.allPlayers ?? null)} in all
          </span>
          <span className="spacer" />
          {status && <span className="dim">checked {ago(status.checkedAt)}</span>}
        </div>
        <div className="modal-tabs" role="tablist">
          {(
            [
              ['osrs', 'Old School'],
              ['rs3', 'RuneScape 3'],
            ] as [Game, string][]
          ).map(([id, label]) => (
            <button key={id} role="tab" aria-selected={game === id} className={`tab ${game === id ? 'active' : ''}`} onClick={() => setGame(id)}>
              {label}
            </button>
          ))}
        </div>
        <div className="modal-body news-list selectable">
          {error && <p style={{ color: 'var(--danger)' }}>{error}</p>}
          {!list && !error && <p className="dim">Loading…</p>}
          {list?.length === 0 && <p className="dim">No news.</p>}
          {list?.map((n, i) => (
            <article key={i} className="news-item">
              <div className="news-meta">
                {n.category && <span className="badge">{n.category}</span>}
                <span className="dim">{newsDate(n.date)}</span>
              </div>
              <h3>{n.title}</h3>
              {n.summary && <p>{n.summary}</p>}
              {n.link && (
                <button className="btn small" onClick={() => api.openNews(n.link).catch((e) => setError(errorText(e)))}>
                  <ExternalLink size={12} /> Read on runescape.com
                </button>
              )}
            </article>
          ))}
        </div>
      </div>
    </div>
  );
}
