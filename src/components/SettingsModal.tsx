import { useState } from 'react';
import { ExternalLink, FolderOpen, KeyRound, Palette, Rocket, Shield, X } from 'lucide-react';
import logo from '../assets/logo.svg';
import { CLIENTS } from '../format';
import { THEMES } from '../themes';
import type { Folder } from '../api';
import type { AppState, ClientId, Settings } from '../types';

type Tab = 'appearance' | 'clients' | 'launching' | 'about';

interface Props {
  state: AppState;
  onPreviewTheme: (id: string) => void;
  onSave: (s: Settings) => void;
  onClose: () => void;
  onOpenFolder: (which: Folder) => void;
}

const TABS: { id: Tab; label: string; icon: typeof Palette }[] = [
  { id: 'appearance', label: 'Appearance', icon: Palette },
  { id: 'clients', label: 'Clients', icon: Rocket },
  { id: 'launching', label: 'Launching', icon: KeyRound },
  { id: 'about', label: 'Privacy & about', icon: Shield },
];

export default function SettingsModal({ state, onPreviewTheme, onSave, onClose, onOpenFolder }: Props) {
  const [tab, setTab] = useState<Tab>('appearance');
  const [s, setS] = useState<Settings>(state.settings);
  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => setS((old) => ({ ...old, [k]: v }));
  const setOpts = (c: ClientId, k: 'jvmArgs' | 'args', v: string) => setS((old) => ({ ...old, [c]: { ...old[c], [k]: v } }));

  const cancel = () => {
    onPreviewTheme(state.settings.theme);
    onClose();
  };

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && cancel()}>
      <div className="modal" role="dialog" aria-modal="true" aria-label="Settings" onKeyDown={(e) => e.key === 'Escape' && cancel()}>
        <div className="modal-head">
          <h2>Settings</h2>
          <button className="icon-btn" title="Close" onClick={cancel}>
            <X size={16} />
          </button>
        </div>
        <div className="modal-tabs" role="tablist">
          {TABS.map((t) => (
            <button key={t.id} role="tab" aria-selected={tab === t.id} className={`tab ${tab === t.id ? 'active' : ''}`} onClick={() => setTab(t.id)}>
              <t.icon size={14} /> {t.label}
            </button>
          ))}
        </div>

        <div className="modal-body">
          {tab === 'appearance' && (
            <>
              <div className="field">
                <span className="label">Theme</span>
                <div className="themes">
                  {THEMES.map((t) => (
                    <button
                      key={t.id}
                      className={`theme-card ${s.theme === t.id ? 'selected' : ''}`}
                      style={{ background: t.colors.panel, color: t.colors.text, borderColor: t.colors.border }}
                      onClick={() => {
                        set('theme', t.id);
                        onPreviewTheme(t.id);
                      }}
                    >
                      <b>{t.name}</b>
                      <span className="swatches">
                        {[t.colors.bg, t.colors.accent, t.colors.accent2, t.colors.success, t.colors.danger].map((c, i) => (
                          <span key={i} style={{ background: c }} />
                        ))}
                      </span>
                    </button>
                  ))}
                </div>
              </div>
              <label className="check">
                <input type="checkbox" checked={s.reduceMotion} onChange={(e) => set('reduceMotion', e.target.checked)} />
                <span>
                  Reduce motion
                  <span className="hint dim">Turns off the floating logo, progress animations and transitions.</span>
                </span>
              </label>
            </>
          )}

          {tab === 'clients' && (
            <>
              {(['runelite', 'hdos', 'rs3'] as ClientId[]).map((c) => (
                <div className="group" key={c}>
                  <h3>
                    <span className="client-mark" style={{ width: 24, height: 24, fontSize: 10, borderRadius: 6 }}>
                      {CLIENTS[c].mark}
                    </span>
                    {CLIENTS[c].name}
                  </h3>
                  <div className="field-row">
                    {c !== 'rs3' && (
                      <div className="field">
                        <label htmlFor={`${c}-jvm`}>JVM options</label>
                        <input
                          id={`${c}-jvm`}
                          className="input mono"
                          placeholder="-Xmx2G"
                          value={s[c].jvmArgs}
                          onChange={(e) => setOpts(c, 'jvmArgs', e.target.value)}
                        />
                      </div>
                    )}
                    <div className="field" style={c === 'rs3' ? { gridColumn: '1 / -1' } : undefined}>
                      <label htmlFor={`${c}-args`}>Client arguments</label>
                      <input
                        id={`${c}-args`}
                        className="input mono"
                        placeholder={c === 'runelite' ? '--insecure-write-credentials' : ''}
                        value={s[c].args}
                        onChange={(e) => setOpts(c, 'args', e.target.value)}
                      />
                    </div>
                  </div>
                  {c === 'rs3' && (
                    <div className="field">
                      <label htmlFor="rs3-uri">Config URI</label>
                      <input
                        id="rs3-uri"
                        className="input mono"
                        placeholder="Default (Jagex live servers)"
                        value={s.rs3ConfigUri}
                        onChange={(e) => set('rs3ConfigUri', e.target.value)}
                      />
                      <span className="hint">Leave empty unless you know you need a different world config.</span>
                    </div>
                  )}
                </div>
              ))}
              <div className="field">
                <label htmlFor="java">Java executable</label>
                <input
                  id="java"
                  className="input mono"
                  placeholder={state.flatpak ? '/app/jre/bin/java (bundled)' : 'Auto: $JAVA_HOME or java on PATH'}
                  value={s.javaPath}
                  onChange={(e) => set('javaPath', e.target.value)}
                />
                <span className="hint">Used for RuneLite and HDOS. Java 17 or newer is recommended.</span>
              </div>
            </>
          )}

          {tab === 'launching' && (
            <>
              <label className="check">
                <input type="checkbox" checked={s.minimizeOnLaunch} onChange={(e) => set('minimizeOnLaunch', e.target.checked)} />
                <span>
                  Minimise Lodestone after launching a game
                  <span className="hint dim">Running games keep going if you close Lodestone.</span>
                </span>
              </label>
              <div className="field">
                <label htmlFor="wrapper">Launch wrapper</label>
                <input
                  id="wrapper"
                  className="input mono"
                  placeholder="gamemoderun %command%"
                  value={s.wrapper}
                  onChange={(e) => set('wrapper', e.target.value)}
                />
                <span className="hint">
                  Runs games through another program, such as <code>gamemoderun</code> or <code>mangohud</code>. <code>%command%</code> marks where the game
                  goes; without it the game is appended.
                </span>
              </div>
              <div className="field">
                <label htmlFor="home">Game home folder</label>
                <input
                  id="home"
                  className="input mono"
                  placeholder={state.flatpak ? `${state.dataDir}/home` : 'Your home folder'}
                  value={s.gameHome}
                  onChange={(e) => set('gameHome', e.target.value)}
                />
                <span className="hint">Where clients keep their settings and caches (for example ~/.runelite). Leave empty for the default.</span>
              </div>
              <div className="field">
                <span className="label">Folders</span>
                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                  <button className="btn" onClick={() => onOpenFolder('logs')}>
                    <FolderOpen size={14} /> Game logs
                  </button>
                  <button className="btn" onClick={() => onOpenFolder('clients')}>
                    <FolderOpen size={14} /> Clients
                  </button>
                  <button className="btn" onClick={() => onOpenFolder('data')}>
                    <FolderOpen size={14} /> Data
                  </button>
                </div>
              </div>
            </>
          )}

          {tab === 'about' && (
            <>
              <div className="about">
                <img src={logo} alt="" />
                <div>
                  <h3>VOID Lodestone</h3>
                  <div className="dim">
                    Version {state.version}
                    {state.flatpak ? ' · Flatpak' : ''} · GPL-3.0-or-later
                  </div>
                  <div className="dim">A fast, open-source Linux launcher for RuneLite, HDOS and RuneScape 3.</div>
                </div>
              </div>
              <div className="group">
                <h3>
                  <Shield size={14} /> Privacy
                </h3>
                <ul className="list">
                  <li>
                    No telemetry, analytics or crash reporting. Lodestone talks only to Jagex, GitHub (RuneLite releases), the HDOS CDN and Jagex's RS3 package
                    server.
                  </li>
                  <li>You sign in on Jagex's own page. Your password and the long-lived sign-in tokens are never stored.</li>
                  <li>
                    Game sessions are kept in {state.secretBackend === 'keyring' ? 'your system keyring' : <b>a private file (no keyring service was found)</b>}
                    .
                  </li>
                  <li>Not affiliated with or endorsed by Jagex. RuneScape and Old School RuneScape are trademarks of Jagex Ltd.</li>
                </ul>
              </div>
              <div>
                <button className="btn" onClick={() => onOpenFolder('website')}>
                  <ExternalLink size={14} /> Source code
                </button>
              </div>
            </>
          )}
        </div>

        <div className="modal-foot">
          <button className="btn" onClick={cancel}>
            Cancel
          </button>
          <button className="btn primary" onClick={() => onSave(s)}>
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
