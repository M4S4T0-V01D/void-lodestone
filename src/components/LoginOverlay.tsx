import { ShieldCheck } from 'lucide-react';
import type { LoginStatus } from '../types';

export default function LoginOverlay({ status, onCancel }: { status: LoginStatus | null; onCancel: () => void }) {
  const verifying = status?.phase === 'verifying';
  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-label="Signing in">
      <div className="modal small">
        <div className="login-card">
          <div className="spinner" />
          <b style={{ fontSize: 15 }}>{verifying ? 'Finishing sign-in…' : 'Sign in with Jagex'}</b>
          <span className="dim">
            {status?.message ||
              (verifying ? 'Creating a game session and loading your characters.' : 'Complete the sign-in in the Jagex window that just opened.')}
          </span>
          <span className="dim" style={{ display: 'flex', gap: 6, alignItems: 'center', fontSize: 11.5 }}>
            <ShieldCheck size={13} /> Your password goes only to Jagex. Lodestone never sees it.
          </span>
          <button className="btn" onClick={onCancel}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
