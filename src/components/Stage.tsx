import { useEffect, useState } from 'react';
import { Bookmark, Bug, CheckCircle2, Download, Hammer, Play, Plus, RefreshCw, X } from 'lucide-react';
import logo from '../assets/logo.svg';
import { ago, bytes, characterName, CLIENTS, duration, makePick, playtimeSums, type Row } from '../format';
import type { Account, ClientId, ClientStatus, Game, Group, Pick, Playtime, Progress } from '../types';

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
  /** The selected client's custom jar, if one is set; enables the switch next to Play. */
  jarPath?: string;
  useJar?: boolean;
  onUseJar?: (on: boolean) => void;
  /** With the custom jar: open a debugger port. */
  debug?: boolean;
  onDebug?: (on: boolean) => void;
  /** Custom jars set per client, which extra rows may use. */
  jars?: Partial<Record<ClientId, string>>;
  /** More characters to start along with `pick`, one per extra row; a '' pick is a row with nothing chosen yet. */
  extras?: Row[];
  onExtras?: (extras: Row[]) => void;
  /** Saved groups for this game, with their index in settings. */
  groups?: { n: number; group: Group }[];
  onLoadGroup?: (n: number) => void;
  onPlayGroup?: (n: number) => void;
  onDeleteGroup?: (n: number) => void;
  onSaveGroup?: (name: string) => void;
  /** Build the plugin projects, then start (or restart) these characters. */
  onBuild?: () => void;
  building?: boolean;
  /** Time played by the chosen character, and the start times of its games still running. */
  played?: { entry?: Playtime; live: number[] } | null;
  /** A data: URL of the picture behind this area, how much to darken it (percent) and blur it (px). */
  background?: string;
  backgroundDim?: number;
  backgroundBlur?: number;
}

/** The choices for an extra row's client: each client of the game, and its custom jar if one is set. */
function rowChoices(clients: ClientId[], jars: Partial<Record<ClientId, string>>) {
  return clients.flatMap((c) => [
    { value: c, label: CLIENTS[c].name },
    ...(jars[c] ? [{ value: `${c}+jar`, label: `${CLIENTS[c].name} · custom jar` }] : []),
  ]);
}

function CharacterSelect(p: { accounts: Account[]; value: Pick; taken: Pick[]; onChange: (v: Pick) => void; label: string }) {
  return (
    <select className="select" aria-label={p.label} value={p.value} onChange={(e) => p.onChange(e.target.value)}>
      {!p.value && <option value="">Choose a character…</option>}
      {p.accounts
        .filter((a) => a.characters.length)
        .map((a) => (
          <optgroup key={a.id} label={a.signedIn ? a.label : `${a.label} (sign in again)`}>
            {a.characters.map((c) => {
              const k = makePick(a.id, c.accountId);
              return (
                <option key={c.accountId} value={k} disabled={k !== p.value && p.taken.includes(k)}>
                  {characterName(c.displayName)}
                </option>
              );
            })}
          </optgroup>
        ))}
    </select>
  );
}

/** Updated within this long: say so on the card. */
const RECENT_UPDATE = 48 * 3600;

const GAMES: Record<Game, { title: string; sub: string; clients: ClientId[] }> = {
  osrs: { title: 'Old School', sub: 'Choose your client, pick a character and step into Gielinor.', clients: ['runelite', 'hdos'] },
  rs3: { title: 'RuneScape', sub: 'The modern RuneScape, launched with your Jagex account.', clients: ['rs3'] },
};

function ClientCard(props: { id: ClientId; status?: ClientStatus; progress?: Progress; selected: boolean; onSelect: () => void; onUpdate: () => void }) {
  const info = CLIENTS[props.id];
  const p = props.progress;
  const active = p && p.phase !== 'ready' && p.phase !== 'error';
  const pct = p && p.total ? Math.min(100, (p.received / p.total) * 100) : 0;
  const s = props.status;
  const recent = s?.installed && s.previousVersion && s.installedAt && Date.now() / 1000 - s.installedAt < RECENT_UPDATE;
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
          {s?.installed && s.lastError ? (
            <span className="badge warn" title={s.lastError}>
              update check failed
            </span>
          ) : s?.installed && s.checkedAt ? (
            <span className="badge" title={`Last checked ${ago(s.checkedAt)}`}>
              up to date
            </span>
          ) : null}
          {s?.installed && <span>{bytes(s.size)}</span>}
          {recent ? (
            <span title={`Updated ${ago(s.installedAt)}`}>
              updated {ago(s.installedAt)} from {s.previousVersion}
            </span>
          ) : s?.checkedAt ? (
            <span>checked {ago(s.checkedAt)}</span>
          ) : null}
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
        {p?.phase === 'error' ? (
          <span style={{ color: 'var(--danger)', fontSize: 11 }} className="selectable">
            {p.message}
          </span>
        ) : (
          s?.installed &&
          s.lastError &&
          !active && (
            <span style={{ color: 'var(--warning)', fontSize: 11 }} className="selectable">
              Couldn't check for updates: {s.lastError}. The installed version still works.
            </span>
          )
        )}
      </div>
    </div>
  );
}

export default function Stage(p: Props) {
  const g = GAMES[p.game];
  const playable = p.accounts.filter((a) => a.characters.length);
  const extras = p.extras ?? [];
  const all = [p.pick, ...extras.map((x) => x.pick)].filter(Boolean);
  const expired = all.some((k) => p.accounts.find((a) => a.id === k.split(':')[0])?.signedIn === false);
  const dim = Math.min(95, Math.max(0, p.backgroundDim ?? 55)) / 100;
  const blur = Math.min(30, Math.max(0, p.backgroundBlur ?? 0));
  const choices = rowChoices(g.clients, p.jars ?? {});
  const [naming, setNaming] = useState<string | null>(null);
  const setExtra = (i: number, v: Partial<Row> | null) =>
    p.onExtras?.(v === null ? extras.filter((_, j) => j !== i) : extras.map((x, j) => (j === i ? { ...x, ...v } : x)));
  const saveGroup = () => {
    const name = naming?.trim();
    if (name) p.onSaveGroup?.(name);
    setNaming(null);
  };

  return (
    <main className={`stage ${p.background ? 'has-bg' : ''}`}>
      {p.background && (
        <div
          className="stage-bg"
          aria-hidden
          style={{
            // Darker towards the bottom, where the Play controls sit.
            backgroundImage: `linear-gradient(color-mix(in srgb, var(--bg) ${dim * 100}%, transparent), color-mix(in srgb, var(--bg) ${Math.min(100, dim * 100 + 25)}%, transparent)), url("${p.background}")`,
            filter: blur ? `blur(${blur}px)` : undefined,
            transform: blur ? `scale(${1 + blur / 150})` : undefined,
          }}
        />
      )}
      <div className="stage-scroll">
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
              {!!p.groups?.length && (
                <div className="groups" aria-label="Saved groups">
                  {p.groups.map(({ n, group }) => (
                    <span className="group-chip" key={n}>
                      <button className="group-load" title={`Load “${group.name}” into the rows below`} onClick={() => p.onLoadGroup?.(n)}>
                        <Bookmark size={12} /> {group.name}
                        <span className="dim">×{group.rows.length}</span>
                      </button>
                      <button className="group-act" title={`Start “${group.name}”`} disabled={p.launching} onClick={() => p.onPlayGroup?.(n)}>
                        <Play size={11} fill="currentColor" />
                      </button>
                      <button className="group-act" title={`Delete “${group.name}”`} onClick={() => p.onDeleteGroup?.(n)}>
                        <X size={11} />
                      </button>
                    </span>
                  ))}
                </div>
              )}
              <div className="launch-row">
                <CharacterSelect accounts={p.accounts} value={p.pick} taken={extras.map((x) => x.pick)} onChange={p.onPick} label="Character" />
                {p.jarPath && p.onUseJar && (
                  <div className="jar-switch" role="group" aria-label="Which jar to launch" title={p.jarPath}>
                    <button aria-pressed={!p.useJar} className={!p.useJar ? 'active' : ''} onClick={() => p.onUseJar!(false)}>
                      Normal
                    </button>
                    <button aria-pressed={!!p.useJar} className={p.useJar ? 'active dev' : ''} onClick={() => p.onUseJar!(true)}>
                      Custom jar
                    </button>
                  </div>
                )}
                {p.jarPath && p.useJar && p.onDebug && (
                  <button
                    className={`debug-switch ${p.debug ? 'active' : ''}`}
                    aria-pressed={!!p.debug}
                    title={p.debug ? 'Debugging on: custom-jar games open a JDWP port (shown under Running)' : 'Open a JDWP port for a debugger'}
                    onClick={() => p.onDebug!(!p.debug)}
                  >
                    <Bug size={14} /> Debug
                  </button>
                )}
                <button className="play" onClick={p.onPlay} disabled={!p.pick || p.launching || expired}>
                  <Play size={18} fill="currentColor" />
                  {p.launching ? 'Starting…' : 'Play'}
                  {!p.launching && all.length > 1 && <span className="play-count">×{all.length}</span>}
                </button>
              </div>
              {extras.map((x, i) => (
                <div className="launch-row extra" key={i}>
                  <CharacterSelect
                    accounts={p.accounts}
                    value={x.pick}
                    taken={[p.pick, ...extras.filter((_, j) => j !== i).map((y) => y.pick)]}
                    onChange={(v) => setExtra(i, { pick: v })}
                    label={`Character ${i + 2}`}
                  />
                  {choices.length > 1 && (
                    <select
                      className="select row-client"
                      aria-label={`Client for character ${i + 2}`}
                      value={x.jar && p.jars?.[x.client] ? `${x.client}+jar` : g.clients.includes(x.client) ? x.client : g.clients[0]}
                      onChange={(e) => {
                        const [c, jar] = e.target.value.split('+');
                        setExtra(i, { client: c as ClientId, jar: !!jar });
                      }}
                    >
                      {choices.map((c) => (
                        <option key={c.value} value={c.value}>
                          {c.label}
                        </option>
                      ))}
                    </select>
                  )}
                  <button className="icon-btn" title="Remove this character" onClick={() => setExtra(i, null)}>
                    <X size={14} />
                  </button>
                </div>
              ))}
              <div className="launch-tools">
                {p.onExtras && playable.length > 0 && (
                  <button
                    className="btn ghost small"
                    title="Start another character along with this one"
                    onClick={() => p.onExtras!([...extras, { pick: '', client: p.client, jar: false }])}
                    disabled={p.launching}
                  >
                    <Plus size={13} /> Add
                  </button>
                )}
                {p.onSaveGroup &&
                  p.pick &&
                  (naming === null ? (
                    <button className="btn ghost small" title="Save these rows as a group you can start in one click" onClick={() => setNaming('')}>
                      <Bookmark size={13} /> Save as group
                    </button>
                  ) : (
                    <span className="group-name">
                      <input
                        className="input"
                        autoFocus
                        placeholder="Group name"
                        value={naming}
                        onChange={(e) => setNaming(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') saveGroup();
                          if (e.key === 'Escape') {
                            e.stopPropagation();
                            setNaming(null);
                          }
                        }}
                      />
                      <button className="btn small" disabled={!naming.trim()} onClick={saveGroup}>
                        Save
                      </button>
                      <button className="icon-btn" title="Cancel" onClick={() => setNaming(null)}>
                        <X size={13} />
                      </button>
                    </span>
                  ))}
                <span className="spacer" />
                {p.onBuild && (
                  <button
                    className="btn small"
                    title="Build your plugin projects with Gradle, copy them to the sideloaded plugins and start these characters (restarting any that run)"
                    disabled={p.building || p.launching || !p.pick || expired}
                    onClick={p.onBuild}
                  >
                    <Hammer size={13} /> {p.building ? 'Building…' : 'Build & play'}
                  </button>
                )}
              </div>
              {p.played && <Played {...p.played} />}
              {expired && <span className="dim">{all.length > 1 ? "One of these accounts' sessions has" : "This account's session has"} expired. Use “Sign in again” in the accounts list.</span>}
              {!playable.length && <span className="dim">None of your accounts has a character yet. Create one in-game first, then refresh.</span>}
            </div>
          )}
        </div>
      </div>
    </main>
  );
}

/** Time played by the chosen character; counts up while it runs. */
function Played({ entry, live }: { entry?: Playtime; live: number[] }) {
  const [now, setNow] = useState(() => Date.now() / 1000);
  useEffect(() => {
    if (!live.length) return;
    const t = setInterval(() => setNow(Date.now() / 1000), 30000);
    return () => clearInterval(t);
  }, [live.length]);
  const t = playtimeSums(entry, live, Math.max(now, ...live));
  if (!t.total) return null;
  return (
    <div className="played" title="Counted on this computer only">
      <span>
        Today <b>{duration(t.today)}</b>
      </span>
      <span>
        This week <b>{duration(t.week)}</b>
      </span>
      <span>
        Total <b>{duration(t.total)}</b>
      </span>
      {!!live.length && <span className="badge ok">playing</span>}
    </div>
  );
}
