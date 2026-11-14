import { useState } from 'react';
import { Check, Plus, RotateCcw, Trash2, X } from 'lucide-react';
import type { CharacterInfo, Goal, Profile, WindowSpot } from '../types';

interface Props {
  name: string;
  info: CharacterInfo;
  /** RuneLite's profiles, to pick one this character plays with. */
  profiles: Profile[];
  /** Profiles per character are on, so an empty choice means the character's own. */
  ownProfiles: boolean;
  window?: WindowSpot;
  onSave: (info: CharacterInfo) => void;
  onForgetWindow: () => void;
  onClose: () => void;
}

export const EMPTY_INFO: CharacterInfo = { notes: '', goals: [], tags: [], color: '', profile: '' };

/** Colours offered before the custom picker. */
const COLORS = ['#d4a64a', '#e06c5a', '#5fb36b', '#4f9fd8', '#a77bd8', '#d877b0', '#8a9aa8'];

/** Tags from a comma-separated line, without repeats. */
export function parseTags(s: string): string[] {
  const seen = new Set<string>();
  return s
    .split(',')
    .map((t) => t.trim())
    .filter((t) => t && !seen.has(t.toLowerCase()) && seen.add(t.toLowerCase()));
}

/** Notes, goals, tags, a colour and a RuneLite profile for one character. */
export default function CharacterModal(p: Props) {
  const [info, setInfo] = useState<CharacterInfo>({ ...EMPTY_INFO, ...p.info });
  const [tags, setTags] = useState(p.info.tags.join(', '));
  const [goal, setGoal] = useState('');
  const set = <K extends keyof CharacterInfo>(k: K, v: CharacterInfo[K]) => setInfo((o) => ({ ...o, [k]: v }));
  const setGoals = (goals: Goal[]) => set('goals', goals);
  const addGoal = () => {
    if (!goal.trim()) return;
    setGoals([...info.goals, { text: goal.trim(), done: false }]);
    setGoal('');
  };
  const save = () => p.onSave({ ...info, tags: parseTags(tags), goals: [...info.goals, ...(goal.trim() ? [{ text: goal.trim(), done: false }] : [])] });
  const known = p.profiles.some((x) => x.name === info.profile);

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && p.onClose()}>
      <div
        className="modal character-modal"
        role="dialog"
        aria-modal="true"
        aria-label={`About ${p.name}`}
        onKeyDown={(e) => e.key === 'Escape' && p.onClose()}
      >
        <div className="modal-head">
          <span className="avatar" style={info.color ? { background: info.color, color: '#111', borderColor: info.color } : undefined}>
            {p.name.charAt(0).toUpperCase()}
          </span>
          <h2>{p.name}</h2>
          <button className="icon-btn" title="Close" onClick={p.onClose}>
            <X size={16} />
          </button>
        </div>
        <div className="modal-body">
          <div className="field">
            <span className="label">Colour</span>
            <div className="colors">
              <button className={`swatch none ${!info.color ? 'selected' : ''}`} title="No colour" onClick={() => set('color', '')} />
              {COLORS.map((c) => (
                <button
                  key={c}
                  className={`swatch ${info.color === c ? 'selected' : ''}`}
                  style={{ background: c }}
                  title={c}
                  onClick={() => set('color', c)}
                />
              ))}
              <input type="color" aria-label="Another colour" value={info.color || '#d4a64a'} onChange={(e) => set('color', e.target.value)} />
            </div>
          </div>
          <div className="field">
            <label htmlFor="char-tags">Tags</label>
            <input id="char-tags" className="input" placeholder="ironman, skiller, pker" value={tags} onChange={(e) => setTags(e.target.value)} />
            <span className="hint">Separated by commas. They show next to the character's name.</span>
          </div>
          <div className="field">
            <span className="label">Goals</span>
            {info.goals.map((g, i) => (
              <div className="goal" key={i}>
                <label className="check">
                  <input
                    type="checkbox"
                    checked={g.done}
                    onChange={(e) => setGoals(info.goals.map((x, j) => (j === i ? { ...x, done: e.target.checked } : x)))}
                  />
                  <span className={g.done ? 'done' : undefined}>{g.text}</span>
                </label>
                <button className="icon-btn danger" title="Remove this goal" onClick={() => setGoals(info.goals.filter((_, j) => j !== i))}>
                  <Trash2 size={13} />
                </button>
              </div>
            ))}
            <div className="goal-add">
              <input
                className="input"
                placeholder="Quest X next, 99 Fishing…"
                value={goal}
                maxLength={200}
                onChange={(e) => setGoal(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addGoal()}
              />
              <button className="btn small" disabled={!goal.trim()} onClick={addGoal}>
                <Plus size={13} /> Add
              </button>
            </div>
          </div>
          <div className="field">
            <label htmlFor="char-notes">Notes</label>
            <textarea id="char-notes" className="input" rows={4} value={info.notes} onChange={(e) => set('notes', e.target.value)} />
          </div>
          <div className="field">
            <label htmlFor="char-profile">RuneLite profile</label>
            <select id="char-profile" className="select" value={info.profile} onChange={(e) => set('profile', e.target.value)}>
              <option value="">{p.ownProfiles ? `Its own (“${p.name}”)` : 'RuneLite’s active profile'}</option>
              {p.profiles.map((x) => (
                <option key={x.id} value={x.name}>
                  {x.name}
                  {x.active ? ' (active)' : ''}
                  {x.sync ? ' · synced' : ''}
                </option>
              ))}
              {info.profile && !known && <option value={info.profile}>{info.profile} (made on first start)</option>}
            </select>
            <span className="hint">
              Which of RuneLite's settings profiles this character starts with. Characters can share one; Settings › Characters copies settings between them.
            </span>
          </div>
          <div className="field">
            <span className="label">Game window</span>
            <div className="field-row" style={{ alignItems: 'center' }}>
              <span className="dim">
                {p.window
                  ? p.window.maximized
                    ? 'Remembered: maximised.'
                    : `Remembered: ${p.window.width}×${p.window.height} at ${p.window.x}, ${p.window.y}.`
                  : 'Not remembered yet. It’s saved while the game runs.'}
              </span>
              {p.window && (
                <button className="btn small" style={{ justifySelf: 'end' }} onClick={p.onForgetWindow}>
                  <RotateCcw size={12} /> Forget
                </button>
              )}
            </div>
          </div>
        </div>
        <div className="modal-foot">
          <button className="btn" onClick={p.onClose}>
            Cancel
          </button>
          <button className="btn primary" onClick={save}>
            <Check size={14} /> Save
          </button>
        </div>
      </div>
    </div>
  );
}
