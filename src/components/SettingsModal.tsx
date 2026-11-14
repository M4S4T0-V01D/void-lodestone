import { useEffect, useRef, useState } from 'react';
import { ExternalLink, FolderOpen, KeyRound, Palette, RefreshCw, Rocket, RotateCcw, Shield, X } from 'lucide-react';
import logo from '../assets/logo.svg';
import { CLIENTS } from '../format';
import { findTheme, THEMES } from '../themes';
import type { Folder } from '../api';
import type { AppState, ClientId, ClientOpts, Settings } from '../types';

type Tab = 'appearance' | 'clients' | 'launching' | 'about';

interface Props {
  state: AppState;
  /** Show these appearance settings while the window is open; null when they're dropped. */
  onPreview: (look: Partial<Settings>) => void;
  onSave: (s: Settings) => void;
  onClose: () => void;
  onOpenFolder: (which: Folder) => void;
  /** Look for a newer Lodestone now. */
  onCheckUpdate: () => void;
}

const TABS: { id: Tab; label: string; icon: typeof Palette }[] = [
  { id: 'appearance', label: 'Appearance', icon: Palette },
  { id: 'clients', label: 'Clients', icon: Rocket },
  { id: 'launching', label: 'Launching', icon: KeyRound },
  { id: 'about', label: 'Privacy & about', icon: Shield },
];

/** The settings shown on the window as they're changed, before Save. */
const LOOK = [
  'theme',
  'accent',
  'uiScale',
  'density',
  'radius',
  'showHero',
  'backgroundDim',
  'backgroundBlur',
  'backgroundPosition',
  'reduceMotion',
] as const satisfies readonly (keyof Settings)[];

function lookOf(s: Settings): Partial<Settings> {
  return Object.fromEntries(LOOK.map((k) => [k, s[k]]));
}

function Slider(p: {
  id: string;
  label: string;
  min: number;
  max: number;
  step: number;
  value: number;
  unit: string;
  disabled?: boolean;
  onChange: (v: number) => void;
}) {
  return (
    <div className="slider">
      <label htmlFor={p.id}>{p.label}</label>
      <input
        id={p.id}
        type="range"
        min={p.min}
        max={p.max}
        step={p.step}
        value={p.value}
        disabled={p.disabled}
        onChange={(e) => p.onChange(Number(e.target.value))}
      />
      <span className="mono dim">
        {p.value}
        {p.unit}
      </span>
    </div>
  );
}

export default function SettingsModal({ state, onPreview, onSave, onClose, onOpenFolder, onCheckUpdate }: Props) {
  const [tab, setTab] = useState<Tab>('appearance');
  const [s, setS] = useState<Settings>(state.settings);
  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => setS((old) => ({ ...old, [k]: v }));
  const setOpts = (c: ClientId, k: Exclude<keyof ClientOpts, 'useJar' | 'debug'>, v: string) =>
    // Setting a jar turns the switch next to Play on; clearing it turns it off.
    setS((old) => ({ ...old, [c]: { ...old[c], [k]: v, ...(k === 'jarPath' && { useJar: !!v.trim() }) } }));

  const cancel = onClose;
  const hasPicture = !!(s.background.trim() || s.backgroundRs3.trim() || s.backgroundRotate);

  useEffect(() => {
    onPreview(lookOf(s));
  }, [s]); // eslint-disable-line react-hooks/exhaustive-deps

  // Escape closes from anywhere, not only while something inside has focus.
  const cancelRef = useRef(cancel);
  cancelRef.current = cancel;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !e.defaultPrevented) cancelRef.current();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && cancel()}>
      <div className="modal" role="dialog" aria-modal="true" aria-label="Settings">
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
                      onClick={() => set('theme', t.id)}
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
              <div className="field">
                <label htmlFor="accent">Accent colour</label>
                <div className="accent-row">
                  <input id="accent" type="color" value={s.accent || findTheme(s.theme).colors.accent} onChange={(e) => set('accent', e.target.value)} />
                  <span className="mono dim">{s.accent || 'The theme’s own'}</span>
                  {s.accent && (
                    <button className="btn small" onClick={() => set('accent', '')}>
                      <RotateCcw size={12} /> Use the theme’s
                    </button>
                  )}
                </div>
              </div>

              <div className="group">
                <h3>Layout</h3>
                <Slider id="ui-scale" label="Size" min={80} max={140} step={5} value={s.uiScale} unit="%" onChange={(v) => set('uiScale', v)} />
                <Slider id="radius" label="Corners" min={0} max={20} step={1} value={s.radius} unit="px" onChange={(v) => set('radius', v)} />
                <div className="field-row">
                  <div className="field">
                    <label htmlFor="density">Spacing</label>
                    <select id="density" className="select" value={s.density} onChange={(e) => set('density', e.target.value as Settings['density'])}>
                      <option value="comfortable">Comfortable</option>
                      <option value="compact">Compact</option>
                    </select>
                  </div>
                  <label className="check" style={{ alignSelf: 'end', paddingBottom: 6 }}>
                    <input type="checkbox" checked={s.showHero} onChange={(e) => set('showHero', e.target.checked)} />
                    <span>Show the logo and title</span>
                  </label>
                </div>
                <div className="field-row">
                  <label className="check">
                    <input type="checkbox" checked={!s.hideAccounts} onChange={(e) => set('hideAccounts', !e.target.checked)} />
                    <span>Accounts panel</span>
                  </label>
                  <label className="check">
                    <input type="checkbox" checked={!s.hideInstances} onChange={(e) => set('hideInstances', !e.target.checked)} />
                    <span>Running games panel</span>
                  </label>
                </div>
                <span className="hint dim" style={{ fontSize: 11.5 }}>
                  The buttons at both ends of the top bar fold the panels away too.
                </span>
              </div>

              <div className="group">
                <h3>Background</h3>
                <div className="field">
                  <label htmlFor="bg">Picture</label>
                  <input
                    id="bg"
                    className="input mono"
                    placeholder="None (for example ~/Pictures/gielinor.png)"
                    value={s.background}
                    onChange={(e) => set('background', e.target.value)}
                  />
                  <span className="hint">
                    Shown behind the middle area. PNG, JPEG, WebP or GIF; it stays on this computer.
                    {state.flatpak && ' The Flatpak can read your Pictures and Documents folders and RuneLite’s screenshots.'}
                  </span>
                </div>
                <div className="field">
                  <label htmlFor="bg-rs3">RuneScape 3 picture</label>
                  <input
                    id="bg-rs3"
                    className="input mono"
                    placeholder="Same as above"
                    value={s.backgroundRs3}
                    onChange={(e) => set('backgroundRs3', e.target.value)}
                  />
                </div>
                <Slider
                  id="bg-dim"
                  label="Darken"
                  min={0}
                  max={90}
                  step={5}
                  value={s.backgroundDim}
                  unit="%"
                  disabled={!hasPicture}
                  onChange={(v) => set('backgroundDim', v)}
                />
                <Slider
                  id="bg-blur"
                  label="Blur"
                  min={0}
                  max={30}
                  step={1}
                  value={s.backgroundBlur}
                  unit="px"
                  disabled={!hasPicture}
                  onChange={(v) => set('backgroundBlur', v)}
                />
                <div className="field-row">
                  <div className="field">
                    <label htmlFor="bg-pos">Keep in view</label>
                    <select
                      id="bg-pos"
                      className="select"
                      value={s.backgroundPosition}
                      disabled={!hasPicture}
                      onChange={(e) => set('backgroundPosition', e.target.value as Settings['backgroundPosition'])}
                    >
                      <option value="center">The middle</option>
                      <option value="top">The top</option>
                      <option value="bottom">The bottom</option>
                    </select>
                  </div>
                  <div className="field" style={{ justifyContent: 'end' }}>
                    <button className="btn" style={{ alignSelf: 'flex-start' }} onClick={() => onOpenFolder('screenshots')}>
                      <FolderOpen size={14} /> RuneLite screenshots
                    </button>
                  </div>
                </div>
                <label className="check">
                  <input type="checkbox" checked={s.backgroundRotate} onChange={(e) => set('backgroundRotate', e.target.checked)} />
                  <span>
                    Rotate RuneLite screenshots
                    <span className="hint dim">On the Old School tab, shows a random screenshot of yours instead of the picture.</span>
                  </span>
                </label>
                {s.backgroundRotate && (
                  <Slider
                    id="rotate-min"
                    label="Change every"
                    min={1}
                    max={60}
                    step={1}
                    value={s.rotateMinutes}
                    unit=" min"
                    onChange={(v) => set('rotateMinutes', v)}
                  />
                )}
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
                  {c !== 'rs3' && (
                    <div className="field">
                      <label htmlFor={`${c}-jar`}>Custom jar</label>
                      <input
                        id={`${c}-jar`}
                        className="input mono"
                        placeholder={`Default (downloaded ${CLIENTS[c].name})`}
                        value={s[c].jarPath}
                        onChange={(e) => setOpts(c, 'jarPath', e.target.value)}
                      />
                      <span className="hint">
                        Runs this jar instead, for example a developer-mode bootstrap.
                        {state.flatpak && ' The Flatpak can read jars in your Documents folder.'}
                      </span>
                      {c === 'runelite' && (
                        <button className="btn" style={{ alignSelf: 'flex-start', marginTop: 4 }} onClick={() => onOpenFolder('plugins')}>
                          <FolderOpen size={14} /> Sideloaded plugins
                        </button>
                      )}
                    </div>
                  )}
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
              <div className="group">
                <h3>Plugin development</h3>
                <div className="field">
                  <label htmlFor="projects">Plugin projects</label>
                  <textarea
                    id="projects"
                    className="input mono"
                    rows={3}
                    placeholder={'~/Documents/Projects/death-recap\n~/Documents/Projects/gielinor-unwrapped'}
                    value={s.pluginProjects}
                    onChange={(e) => set('pluginProjects', e.target.value)}
                  />
                  <span className="hint">
                    One Gradle project per line. Build &amp; play builds each one, copies the jars to the sideloaded plugins folder and starts (or restarts)
                    your characters only if every build worked.
                  </span>
                </div>
                <div className="field-row">
                  <div className="field">
                    <label htmlFor="task">Gradle task</label>
                    <input id="task" className="input mono" placeholder="jar" value={s.buildTask} onChange={(e) => set('buildTask', e.target.value)} />
                  </div>
                  <div className="field">
                    <label htmlFor="debug-port">Debug port</label>
                    <input
                      id="debug-port"
                      className="input mono"
                      type="number"
                      min={1024}
                      max={65535}
                      value={s.debugPort}
                      onChange={(e) => set('debugPort', Number(e.target.value) || 5005)}
                    />
                  </div>
                </div>
                <span className="hint">The Debug switch opens a JDWP port for your IDE to attach to; each extra game takes the next free port.</span>
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
              <label className="check">
                <input type="checkbox" checked={s.closeToTray} disabled={!state.tray} onChange={(e) => set('closeToTray', e.target.checked)} />
                <span>
                  Close to the tray while games run
                  <span className="hint dim">
                    {state.tray
                      ? 'The tray icon lists running games, starts saved groups and stops everything.'
                      : 'No tray is available here (libappindicator is missing).'}
                  </span>
                </span>
              </label>
              <label className="check">
                <input type="checkbox" checked={s.notifications} onChange={(e) => set('notifications', e.target.checked)} />
                <span>
                  Desktop notifications
                  <span className="hint dim">For crashes, expired sessions, new Lodestone versions and update checks that fail.</span>
                </span>
              </label>
              <label className="check">
                <input type="checkbox" checked={s.checkUpdates} onChange={(e) => set('checkUpdates', e.target.checked)} />
                <span>
                  Check for new versions of Lodestone
                  <span className="hint dim">Asks GitHub for the latest release at start-up and every six hours.</span>
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
                  <button className="btn" onClick={() => onOpenFolder('plugins')}>
                    <FolderOpen size={14} /> RuneLite plugins
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
                    No telemetry, analytics or crash reporting. Lodestone talks only to Jagex, GitHub (RuneLite and Lodestone releases), the HDOS CDN and
                    Jagex's RS3 package server.
                  </li>
                  <li>You sign in on Jagex's own page. Your password and the long-lived sign-in tokens are never stored.</li>
                  <li>
                    Game sessions are kept in {state.secretBackend === 'keyring' ? 'your system keyring' : <b>a private file (no keyring service was found)</b>}
                    .
                  </li>
                  <li>Not affiliated with or endorsed by Jagex. RuneScape and Old School RuneScape are trademarks of Jagex Ltd.</li>
                </ul>
              </div>
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                <button className="btn" onClick={onCheckUpdate}>
                  <RefreshCw size={14} /> Check for updates
                </button>
                <button className="btn" onClick={() => onOpenFolder('releases')}>
                  <ExternalLink size={14} /> Releases
                </button>
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
