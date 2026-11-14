import { Download, ExternalLink, RotateCw, X } from 'lucide-react';
import { bytes, noteBlocks } from '../format';
import type { Release, UpdateProgress } from '../types';

interface Props {
  release: Release;
  /** "update": a newer version to install; "news": what's new in the version that runs now. */
  mode: 'update' | 'news';
  current: string;
  progress: UpdateProgress | null;
  /** Games are running, so restarting now would take them down. */
  gamesRunning: boolean;
  onInstall: () => void;
  onRestart: () => void;
  onOpenPage: () => void;
  onClose: () => void;
}

export function ReleaseNotes({ notes }: { notes: string }) {
  const blocks = noteBlocks(notes);
  if (!blocks.length) return <p className="dim">No release notes.</p>;
  return (
    <>
      {blocks.map((b, i) =>
        b.kind === 'list' ? (
          <ul key={i} className="list">
            {b.items.map((t, j) => (
              <li key={j}>{t}</li>
            ))}
          </ul>
        ) : b.kind === 'heading' ? (
          <h3 key={i}>{b.text}</h3>
        ) : (
          <p key={i}>{b.text}</p>
        ),
      )}
    </>
  );
}

/** A new Lodestone release: its notes, and installing it (Flatpak) or opening its page (elsewhere). */
export default function UpdateModal({ release, mode, current, progress, gamesRunning, onInstall, onRestart, onOpenPage, onClose }: Props) {
  const working = progress?.phase === 'downloading' || progress?.phase === 'installing';
  const done = progress?.phase === 'done';
  const pct = progress && progress.total ? Math.round((progress.received / progress.total) * 100) : 0;

  return (
    <div className="overlay" onClick={(e) => e.target === e.currentTarget && !working && onClose()}>
      <div
        className="modal update-modal"
        role="dialog"
        aria-modal="true"
        aria-label={mode === 'news' ? "What's new" : 'Update'}
        onKeyDown={(e) => e.key === 'Escape' && !working && onClose()}
      >
        <div className="modal-head">
          <h2>{mode === 'news' ? `What's new in ${release.version}` : `Lodestone ${release.version} is out`}</h2>
          {!working && (
            <button className="icon-btn" title="Close" onClick={onClose}>
              <X size={16} />
            </button>
          )}
        </div>
        <div className="modal-body update-notes selectable">
          {mode === 'update' && <p className="dim">You have {current}.</p>}
          <ReleaseNotes notes={release.notes} />
        </div>
        <div className="modal-foot">
          <span className="update-status" role="status">
            {progress?.phase === 'downloading' && `Downloading… ${progress.total ? `${pct}% of ${bytes(progress.total)}` : bytes(progress.received)}`}
            {progress?.phase === 'installing' && 'Installing…'}
            {done && (gamesRunning ? 'Installed. Restart Lodestone once your games are done.' : 'Installed. Restart to use it.')}
            {progress?.phase === 'error' && <span style={{ color: 'var(--danger)' }}>{progress.message}</span>}
          </span>
          {mode === 'news' ? (
            <button className="btn primary" onClick={onClose}>
              Close
            </button>
          ) : done ? (
            <button className="btn primary" disabled={gamesRunning} onClick={onRestart} title={gamesRunning ? 'Stop your games first' : undefined}>
              <RotateCw size={14} /> Restart now
            </button>
          ) : (
            <>
              <button className="btn" onClick={onOpenPage} disabled={working}>
                <ExternalLink size={14} /> Release page
              </button>
              {release.installable && (
                <button className="btn primary" onClick={onInstall} disabled={working}>
                  <Download size={14} /> {progress?.phase === 'error' ? 'Try again' : 'Install'}
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
