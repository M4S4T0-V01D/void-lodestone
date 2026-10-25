import { CheckCircle2, Download, Play, Plus, RefreshCw } from 'lucide-react';
import logo from '../assets/logo.svg';
import { ago, bytes, characterName, CLIENTS, makePick } from '../format';
import type { Account, ClientId, ClientStatus, Game, Pick, Progress } from '../types';

interface Props {
  game: Game;
  client: ClientId;
  accounts: Account[];
  clients: ClientStatus[];
  progress: Partial<Record<ClientId, Progress>>;
  pick: Pick;
  launching: boolean;
  busy: boolean;
  onClient: (c: ClientId) => void;
  onPick: (p: Pick) => void;
  onPlay: () => void;
  onUpdate: (c: ClientId) => void;
  onAdd: () => void;
}

const GAMES: Record<Game, { title: string; sub: string; clients: ClientId[] }> = {
  osrs: { title: 'Old School', sub: 'Choose your client, pick a character and step into Gielinor.', clients: ['runelite', 'hdos'] },
  rs3: { title: 'RuneScape', sub: 'The modern RuneScape, launched with your Jagex account.', clients: ['rs3'] },
};

function ClientCard(props: { id: ClientId; status?: ClientStatus; progress?: Progress; selected: boolean; onSelect: () => void; onUpdate: () => void }) {
  const info = CLIENTS[props.id];
  const p = props.progress;
  const active = p && p.phase !== 'ready' && p.phase !== 'error';
  const pct = p && p.total ? Math.min(100, (p.received / p.total) * 100) : 0;
  return (
    <div
      className={`client ${props.selected ? 'selected' : ''}`}
      role="radio"
      aria-checked={props.selected}
      tabIndex={0}
      onClick={props.onSelect}
      onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && props.onSelect()}
    >
      {props.selected && <CheckCircle2 className="client-check" size={16} />}
      <span className="client-mark">{info.mark}</span>
      <div className="client-body">
        <span className="client-name">{info.name}</span>
        <span className="client-blurb">{info.blurb}</span>
        <div className="client-meta">
          {props.status?.installed ? (
            <span className="badge ok">{props.status.version || 'installed'}</span>
          ) : (
            <span className="badge warn">downloads on first launch</span>
          )}
          {props.status?.installed && <span>{bytes(props.status.size)}</span>}
          {props.status?.checkedAt ? <span>checked {ago(props.status.checkedAt)}</span> : null}
          <span className="spacer" />
          <button
            className="icon-btn"
            title={props.status?.installed ? 'Check for updates' : 'Download now'}
            disabled={!!active}
            onClick={(e) => {
              e.stopPropagation();
              props.onUpdate();
            }}
          >
            {props.status?.installed ? <RefreshCw size={13} /> : <Download size={13} />}
          </button>
        </div>
        {active && (
          <>
            <div className={`bar ${p.total ? '' : 'indeterminate'}`}>
              <div style={p.total ? { width: `${pct}%` } : undefined} />
            </div>
            <span className="dim" style={{ fontSize: 11 }}>
              {p.phase === 'downloading' ? `Downloading ${bytes(p.received)}${p.total ? ` of ${bytes(p.total)}` : ''}` : p.message || p.phase}
            </span>
          </>
        )}
        {p?.phase === 'error' && (
          <span style={{ color: 'var(--danger)', fontSize: 11 }} className="selectable">
            {p.message}
          </span>
        )}
      </div>
    </div>
  );
}

export default function Stage(p: Props) {
  const g = GAMES[p.game];
  const playable = p.accounts.filter((a) => a.characters.length);
  const [accId] = p.pick.split(':');
  const account = p.accounts.find((a) => a.id === accId);
  const expired = account && !account.signedIn;

  return (
    <main className="stage">
      <div className="stage-inner">
        <div className="hero">
          <img className="hero-logo" src={logo} alt="" />
          <h1 className="title">{g.title}</h1>
          <span className="subtitle">{g.sub}</span>
        </div>

        <div className="section-label">Client</div>
        <div className="clients" role="radiogroup" aria-label="Client">
          {g.clients.map((c) => (
            <ClientCard
              key={c}
              id={c}
              status={p.clients.find((s) => s.client === c)}
              progress={p.progress[c]}
              selected={p.client === c}
              onSelect={() => p.onClient(c)}
              onUpdate={() => p.onUpdate(c)}
            />
          ))}
        </div>

        {!p.accounts.length ? (
          <div className="welcome">
            <b style={{ color: 'var(--text)' }}>Add a Jagex account to play</b>
            <span>You'll sign in on Jagex's own page in a separate window. Lodestone keeps only the game session it needs, in your system keyring.</span>
            <button className="btn primary" onClick={p.onAdd} disabled={p.busy}>
              <Plus size={14} /> Add account
            </button>
          </div>
        ) : (
          <div className="launchpad">
            <div className="launch-row">
              <select className="select" aria-label="Character" value={p.pick} onChange={(e) => p.onPick(e.target.value)}>
                {!p.pick && <option value="">Choose a character…</option>}
                {playable.map((a) => (
                  <optgroup key={a.id} label={a.signedIn ? a.label : `${a.label} (sign in again)`}>
                    {a.characters.map((c) => (
                      <option key={c.accountId} value={makePick(a.id, c.accountId)}>
                        {characterName(c.displayName)}
                      </option>
                    ))}
                  </optgroup>
                ))}
              </select>
              <button className="play" onClick={p.onPlay} disabled={!p.pick || p.launching || !!expired}>
                <Play size={18} fill="currentColor" />
                {p.launching ? 'Starting…' : 'Play'}
              </button>
            </div>
            {expired && <span className="dim">This account's session has expired. Use “Sign in again” in the accounts list.</span>}
            {!playable.length && <span className="dim">None of your accounts has a character yet. Create one in-game first, then refresh.</span>}
          </div>
        )}
      </div>
    </main>
  );
}
