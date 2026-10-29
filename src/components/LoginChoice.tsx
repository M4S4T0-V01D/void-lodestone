import { AppWindow, Globe, X } from 'lucide-react';
import type { LoginMode } from '../types';

/** Asked before each sign-in: Jagex's pages in Lodestone's own window, or in the user's browser. */
export default function LoginChoice({ again, onPick, onCancel }: { again: boolean; onPick: (m: LoginMode) => void; onCancel: () => void }) {
  return (
    <div className="overlay" onClick={(e) => e.target === e.currentTarget && onCancel()}>
      <div className="modal small" role="dialog" aria-modal="true" aria-label="Choose how to sign in" onKeyDown={(e) => e.key === 'Escape' && onCancel()}>
        <div className="modal-head">
          <h2>{again ? 'Sign in again' : 'Add a Jagex account'}</h2>
          <button className="icon-btn" title="Close" onClick={onCancel}>
            <X size={16} />
          </button>
        </div>
        <div className="login-choice">
          <button className="btn" onClick={() => onPick('browser')}>
            <Globe size={15} /> Browser
          </button>
          <button className="btn primary" onClick={() => onPick('window')} autoFocus>
            <AppWindow size={15} /> Window
          </button>
        </div>
        <span className="dim login-choice-hint">Use Browser if the sign-in window stays blank.</span>
      </div>
    </div>
  );
}
