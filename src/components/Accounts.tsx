import { useState } from 'react';
import { ArrowDown, ArrowUp, Check, LogIn, Pencil, Plus, RefreshCw, Trash2, X } from 'lucide-react';
import { ago, characterName, makePick } from '../format';
import type { Account, Instance, Pick } from '../types';

interface Props {
  accounts: Account[];
  instances: Instance[];
  pick: Pick;
  busy: boolean;
  onPick: (p: Pick) => void;
  onAdd: () => void;
  onReauth: (id: string) => void;
  onRemove: (id: string) => void;
  onRename: (id: string, label: string) => void;
  onMove: (id: string, to: number) => void;
  onRefresh: () => void;
}

/** One line on how the saved session last fared with Jagex. */
function Health({ a }: { a: Account }) {
  const h = a.health;
  const since = a.signedInAt ? `Signed in ${ago(a.signedInAt)}.` : '';
  if (!a.signedIn || !h) return null;
  const [cls, text, tip] =
    h.state === 'offline'
      ? ['warn', "Couldn't reach Jagex", "Jagex didn't answer the last check, so the session is untested. Play still tries it."]
      : ['ok', 'Session OK', 'Jagex accepted the saved session.'];
  return (
    <div className={`health ${cls}`} title={`${tip} ${since}`.trim()}>
      {text} · checked {ago(h.checkedAt)}
    </div>
  );
}

export default function Accounts(p: Props) {
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState('');
  const [confirm, setConfirm] = useState<string | null>(null);

  const running = (accountId: string, name: string) => p.instances.filter((i) => i.accountId === accountId && i.character === characterName(name)).length;

  const commit = (id: string) => {
    if (draft.trim()) p.onRename(id, draft);
    setEditing(null);
  };

  return (
    <aside className="side" aria-label="Accounts">
      <div className="side-head">
        <span>Jagex accounts</span>
        <span className="spacer" />
        <button className="icon-btn" title="Refresh characters and sessions" onClick={p.onRefresh} disabled={p.busy || !p.accounts.length}>
          <RefreshCw size={14} />
        </button>
      </div>
      <div className="side-scroll">
        {!p.accounts.length && <div className="empty">No accounts yet. Add a Jagex account to start playing.</div>}
        {p.accounts.map((a, idx) => (
          <div className="account" key={a.id}>
            <div className="account-head">
              <span
                className={`dot ${!a.signedIn ? 'off' : a.health?.state === 'offline' ? 'warn' : 'on'}`}
                title={a.signedIn ? 'Signed in' : 'Session expired'}
              />
              {editing === a.id ? (
                <input
                  className="input"
                  autoFocus
                  value={draft}
                  maxLength={40}
                  onChange={(e) => setDraft(e.target.value)}
                  onBlur={() => commit(a.id)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') commit(a.id);
                    if (e.key === 'Escape') setEditing(null);
                  }}
                />
              ) : (
                <span className="label" title={a.label}>
                  {a.label}
                </span>
              )}
              {confirm === a.id ? (
                <span className="account-actions" style={{ opacity: 1 }}>
                  <button className="icon-btn danger" title="Remove this account" onClick={() => p.onRemove(a.id)}>
                    <Check size={14} />
                  </button>
                  <button className="icon-btn" title="Keep it" onClick={() => setConfirm(null)}>
                    <X size={14} />
                  </button>
                </span>
              ) : (
                <span className="account-actions">
                  <button className="icon-btn" title="Move up" disabled={idx === 0} onClick={() => p.onMove(a.id, idx - 1)}>
                    <ArrowUp size={13} />
                  </button>
                  <button className="icon-btn" title="Move down" disabled={idx === p.accounts.length - 1} onClick={() => p.onMove(a.id, idx + 1)}>
                    <ArrowDown size={13} />
                  </button>
                  <button
                    className="icon-btn"
                    title="Rename"
                    onClick={() => {
                      setDraft(a.label);
                      setEditing(a.id);
                    }}
                  >
                    <Pencil size={13} />
                  </button>
                  <button className="icon-btn danger" title="Remove" onClick={() => setConfirm(a.id)}>
                    <Trash2 size={13} />
                  </button>
                </span>
              )}
            </div>
            <Health a={a} />
            {!a.signedIn && (
              <div className="signin-note">
                <span title={a.health ? `Checked ${ago(a.health.checkedAt)}` : undefined}>Session expired</span>
                <button className="btn small" onClick={() => p.onReauth(a.id)} disabled={p.busy}>
                  <LogIn size={12} /> Sign in again
                </button>
              </div>
            )}
            {a.characters.map((c) => {
              const id = makePick(a.id, c.accountId);
              const n = running(a.id, c.displayName);
              const name = characterName(c.displayName);
              return (
                <button key={id} className={`char ${p.pick === id ? 'selected' : ''}`} onClick={() => p.onPick(id)} aria-pressed={p.pick === id}>
                  <span className="avatar">{name.charAt(0).toUpperCase()}</span>
                  <span className="name">{name}</span>
                  {n > 0 && <span className="running">● playing</span>}
                </button>
              );
            })}
            {a.signedIn && !a.characters.length && <div className="empty">No characters on this account yet.</div>}
          </div>
        ))}
      </div>
      <div className="side-foot">
        <button className="btn primary wide" onClick={p.onAdd} disabled={p.busy}>
          <Plus size={14} /> Add account
        </button>
      </div>
    </aside>
  );
}
