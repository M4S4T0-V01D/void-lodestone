import { useEffect, useRef } from 'react';
import { X } from 'lucide-react';
import type { BuildLine } from '../types';

export interface Build {
  lines: BuildLine[];
  phase: 'running' | 'ok' | 'failed';
  /** What happened, once it's over. */
  message: string;
}

/** Gradle's output while "Build & play" runs; the games start only if every project builds. */
export default function BuildModal({ build, onCancel, onClose }: { build: Build; onCancel: () => void; onClose: () => void }) {
  const box = useRef<HTMLDivElement>(null);
  const running = build.phase === 'running';
  useEffect(() => {
    if (box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [build.lines.length]);

  return (
    <div className="overlay" onClick={(e) => e.target === e.currentTarget && !running && onClose()}>
      <div className="modal log-modal build-modal" role="dialog" aria-modal="true" aria-label="Build" onKeyDown={(e) => e.key === 'Escape' && !running && onClose()}>
        <div className="modal-head">
          <h2>
            Build & play
            <span className={`badge ${build.phase === 'ok' ? 'ok' : build.phase === 'failed' ? 'bad' : ''}`} style={{ marginLeft: 8 }}>
              {running ? 'building…' : build.phase === 'ok' ? 'built' : 'failed'}
            </span>
          </h2>
          {!running && (
            <button className="icon-btn" title="Close" onClick={onClose}>
              <X size={16} />
            </button>
          )}
        </div>
        <div className="log-view selectable wrap" ref={box}>
          {build.lines.map((l, i) => (
            <div key={i} className={l.err || /FAILED|error:/.test(l.line) ? 'err' : /^> /.test(l.line) ? 'mine' : /warning:/.test(l.line) ? 'warn' : ''}>
              {l.line || ' '}
            </div>
          ))}
          {!build.lines.length && <div className="log-empty">Starting Gradle…</div>}
        </div>
        <div className="modal-foot">
          <span className="selectable" style={{ flex: 1, fontSize: 12, alignSelf: 'center', color: build.phase === 'failed' ? 'var(--danger)' : 'var(--text-dim)' }}>
            {build.message}
          </span>
          {running ? (
            <button className="btn" onClick={onCancel}>
              Cancel build
            </button>
          ) : (
            <button className="btn primary" onClick={onClose}>
              Close
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
