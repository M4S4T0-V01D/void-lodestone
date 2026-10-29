import { useState } from 'react';
import { ShieldCheck } from 'lucide-react';
import { api, errorText } from '../api';
import type { LoginMode, LoginStatus } from '../types';

export default function LoginOverlay({ status, mode, onCancel }: { status: LoginStatus | null; mode: LoginMode; onCancel: () => void }) {
  const verifying = status?.phase === 'verifying';
  const browser = mode === 'browser';
  const [url, setUrl] = useState('');
  const [error, setError] = useState('');

  const submit = async () => {
    if (!url.trim()) return;
    try {
      await api.submitLoginUrl(url);
      setUrl('');
      setError('');
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-label="Signing in">
      <div className="modal small">
        <div className="login-card">
          <div className="spinner" />
          <b style={{ fontSize: 15 }}>{verifying ? 'Finishing sign-in…' : 'Sign in with Jagex'}</b>
          <span className="dim">
            {status?.message ||
              (verifying
                ? 'Creating a game session and loading your characters.'
                : browser
                  ? 'Complete the sign-in in your browser.'
                  : 'Complete the sign-in in the Jagex window that just opened.')}
          </span>
          {browser && !verifying && (
            <form
              className="login-paste"
              onSubmit={(e) => {
                e.preventDefault();
                void submit();
              }}
            >
              <input
                className="input mono"
                placeholder="Paste the address here"
                aria-label="Address from the browser"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                autoFocus
              />
              <button className="btn primary" type="submit" disabled={!url.trim()}>
                Continue
              </button>
            </form>
          )}
          {error && <span className="login-error selectable">{error}</span>}
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
