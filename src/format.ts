import type { Account, ClientId, Game, Pick } from './types';

export const CLIENTS: Record<ClientId, { name: string; game: Game; mark: string; blurb: string }> = {
  runelite: { name: 'RuneLite', game: 'osrs', mark: 'RL', blurb: 'Open-source client with a huge plugin hub.' },
  hdos: { name: 'HDOS', game: 'osrs', mark: 'HD', blurb: 'High-detail graphics for Old School.' },
  rs3: { name: 'RuneScape 3', game: 'rs3', mark: 'RS', blurb: "Jagex's official RuneScape client." },
};

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

/** "just now", "5 min ago", "3 h ago", "2 d ago" for a unix time in seconds. */
export function ago(secs: number, now = Date.now() / 1000): string {
  if (!secs) return 'never';
  const d = Math.max(0, now - secs);
  if (d < 60) return 'just now';
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
  return `${Math.floor(d / 86400)} d ago`;
}

/** "1:05:09" style running time. */
export function uptime(startedAt: number, now = Date.now() / 1000): string {
  const s = Math.max(0, Math.floor(now - startedAt));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const mm = String(m).padStart(h ? 2 : 1, '0');
  return `${h ? `${h}:` : ''}${mm}:${String(sec).padStart(2, '0')}`;
}

export function makePick(accountId: string, characterId: string): Pick {
  return `${accountId}:${characterId}`;
}

export function splitPick(p: Pick): [string, string] | null {
  const i = p.indexOf(':');
  return i > 0 ? [p.slice(0, i), p.slice(i + 1)] : null;
}

/** The saved pick if it still exists, else the first character of the first signed-in account. */
export function resolvePick(saved: Pick, accounts: Account[]): Pick {
  const parts = splitPick(saved);
  if (parts) {
    const a = accounts.find((x) => x.id === parts[0]);
    if (a?.characters.some((c) => c.accountId === parts[1])) return saved;
  }
  const a = accounts.find((x) => x.signedIn && x.characters.length) ?? accounts.find((x) => x.characters.length);
  return a ? makePick(a.id, a.characters[0]!.accountId) : '';
}

export function characterName(name: string): string {
  return name.trim() || 'New character';
}
