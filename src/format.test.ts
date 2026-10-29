import { describe, expect, it } from 'vitest';
import { ago, bytes, duration, isErrorLine, isStackLine, loadRows, makePick, playtimeSums, resolvePick, splitPick, uptime } from './format';
import { THEMES, findTheme, mix, themeVars } from './themes';
import type { Account } from './types';

const acct = (id: string, chars: string[], signedIn = true): Account => ({
  id,
  label: id,
  addedAt: 0,
  signedIn,
  characters: chars.map((c) => ({ accountId: c, displayName: c })),
});

describe('format', () => {
  it('sizes', () => {
    expect(bytes(512)).toBe('512 B');
    expect(bytes(2048)).toBe('2.0 KB');
    expect(bytes(30 * 1024 * 1024)).toBe('30 MB');
  });

  it('relative times', () => {
    expect(ago(0)).toBe('never');
    expect(ago(1000, 1030)).toBe('just now');
    expect(ago(1000, 1000 + 300)).toBe('5 min ago');
    expect(ago(1000, 1000 + 7200)).toBe('2 h ago');
    expect(ago(1000, 1000 + 3 * 86400)).toBe('3 d ago');
  });

  it('uptime', () => {
    expect(uptime(0, 65)).toBe('1:05');
    expect(uptime(0, 3909)).toBe('1:05:09');
  });

  it('picks', () => {
    expect(splitPick(makePick('a', 'c:1'))).toEqual(['a', 'c:1']);
    expect(splitPick('')).toBeNull();
    const accounts = [acct('x', [], true), acct('a', ['c1', 'c2'], false), acct('b', ['d1'])];
    expect(resolvePick('a:c2', accounts)).toBe('a:c2');
    expect(resolvePick('a:gone', accounts)).toBe('b:d1');
    expect(resolvePick('', [acct('a', ['c1'], false)])).toBe('a:c1');
    expect(resolvePick('', [])).toBe('');
  });
});

describe('themes', () => {
  it('has the VOID set with valid colours', () => {
    expect(THEMES.map((t) => t.id)).toEqual(['void', 'nebula', 'abyss', 'phosphor', 'ember', 'daylight', 'high-contrast']);
    for (const t of THEMES) for (const v of Object.values(themeVars(t))) expect(v).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it('falls back to Void', () => {
    expect(findTheme('nope').id).toBe('void');
  });

  it('mixes', () => {
    expect(mix('#000000', '#ffffff', 0.5)).toBe('#808080');
  });
});

describe('extras', () => {
  it('formats durations', () => {
    expect(duration(59)).toBe('0 m');
    expect(duration(3600 + 5 * 60)).toBe('1 h 05 m');
  });

  it('sums playtime by local day and week', () => {
    // Wednesday noon, local time.
    const now = new Date(2026, 9, 28, 12, 0, 0).getTime() / 1000;
    const monday = new Date(2026, 9, 26, 10, 0, 0).getTime() / 1000;
    const lastWeek = new Date(2026, 9, 20, 10, 0, 0).getTime() / 1000;
    const entry = { total: 10000, sessions: [[lastWeek, lastWeek + 600], [monday, monday + 1200], [now - 1800, now - 600]] as [number, number][] };
    const s = playtimeSums(entry, [now - 300], now);
    expect(s.today).toBe(1200 + 300);
    expect(s.week).toBe(1200 + 1200 + 300);
    expect(s.total).toBe(10300);
    expect(playtimeSums(undefined, [], now)).toEqual({ today: 0, week: 0, total: 0 });
  });

  it('reads old and new extra rows', () => {
    expect(loadRows(['a:1', { pick: 'b:2', client: 'hdos', jar: true }, { pick: 'c:3', client: 'nope' }, 5, null])).toEqual([
      { pick: 'a:1', client: 'runelite', jar: false },
      { pick: 'b:2', client: 'hdos', jar: true },
      { pick: 'c:3', client: 'runelite', jar: false },
    ]);
    expect(loadRows('x')).toEqual([]);
  });

  it('spots error and stack lines', () => {
    expect(isErrorLine('2026-10-28 12:00:00 [Client] ERROR n.r.c.Foo - boom')).toBe(true);
    expect(isErrorLine('java.lang.NullPointerException: x')).toBe(true);
    expect(isErrorLine('\tat com.a.B.c(B.java:1)')).toBe(false);
    expect(isErrorLine('INFO loaded ErrorHandler')).toBe(false);
    expect(isStackLine('\tat com.a.B.c(B.java:1)')).toBe(true);
    expect(isStackLine('\t... 12 more')).toBe(true);
  });
});
