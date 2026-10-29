import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import Accounts from './Accounts';
import Stage from './Stage';
import type { Account } from '../types';

const accounts: Account[] = [
  { id: 'a', label: 'Main', addedAt: 0, signedIn: true, health: { state: 'offline', checkedAt: 1 }, characters: [{ accountId: '1', displayName: 'Zezima' }] },
  { id: 'b', label: 'Alt', addedAt: 0, signedIn: false, characters: [{ accountId: '2', displayName: '' }] },
];

const noop = () => {};

afterEach(cleanup);

describe('Accounts', () => {
  it('lists characters, flags expired sessions and picks', () => {
    const onPick = vi.fn();
    const onReauth = vi.fn();
    render(
      <Accounts
        accounts={accounts}
        instances={[]}
        pick="a:1"
        busy={false}
        onPick={onPick}
        onAdd={noop}
        onReauth={onReauth}
        onRemove={noop}
        onRename={noop}
        onMove={noop}
        onRefresh={noop}
      />,
    );
    expect(screen.getByText('Zezima')).toBeTruthy();
    expect(screen.getByText('New character')).toBeTruthy();
    fireEvent.click(screen.getByText('New character'));
    expect(onPick).toHaveBeenCalledWith('b:2');
    fireEvent.click(screen.getByText('Sign in again'));
    expect(onReauth).toHaveBeenCalledWith('b');
  });
  it('shows session health', () => {
    render(<Accounts {...{ accounts, instances: [], pick: '', busy: false, onPick: noop, onAdd: noop, onReauth: noop, onRemove: noop, onRename: noop, onMove: noop, onRefresh: noop }} />);
    expect(screen.getByText(/Couldn't reach Jagex/)).toBeTruthy();
  });
});

describe('Stage', () => {
  const base = {
    client: 'runelite' as const,
    clients: [],
    progress: {},
    launching: false,
    busy: false,
    onClient: noop,
    onPick: noop,
    onUpdate: noop,
    onAdd: noop,
  };

  it('offers only RuneLite and HDOS for Old School', () => {
    render(<Stage {...base} game="osrs" accounts={accounts} pick="a:1" onPlay={noop} />);
    expect(screen.getByText('RuneLite')).toBeTruthy();
    expect(screen.getByText('HDOS')).toBeTruthy();
    expect(screen.queryByText('RuneScape 3')).toBeNull();
  });

  it('disables Play for an expired account', () => {
    render(<Stage {...base} game="rs3" client="rs3" accounts={accounts} pick="b:2" onPlay={noop} />);
    expect((screen.getByText('Play').closest('button') as HTMLButtonElement).disabled).toBe(true);
  });

  it('switches between the normal client and the custom jar', () => {
    const onUseJar = vi.fn();
    render(<Stage {...base} game="osrs" accounts={accounts} pick="a:1" onPlay={noop} jarPath="/x/dev.jar" useJar={false} onUseJar={onUseJar} />);
    fireEvent.click(screen.getByText('Custom jar'));
    expect(onUseJar).toHaveBeenCalledWith(true);
  });

  it('adds more character rows and counts them on Play', () => {
    const onExtras = vi.fn();
    const { rerender } = render(<Stage {...base} game="osrs" accounts={accounts} pick="a:1" onPlay={noop} extras={[]} onExtras={onExtras} />);
    fireEvent.click(screen.getByText('Add'));
    expect(onExtras).toHaveBeenCalledWith([{ pick: '', client: 'runelite', jar: false }]);
    rerender(<Stage {...base} game="osrs" accounts={accounts} pick="a:1" onPlay={noop} extras={[{ pick: 'b:2', client: 'runelite', jar: false }]} onExtras={onExtras} />);
    expect(screen.getByText('×2')).toBeTruthy();
    // b's session has expired, so Play waits for it to be signed in again or removed.
    expect((screen.getByText('Play').closest('button') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTitle('Remove this character'));
    expect(onExtras).toHaveBeenLastCalledWith([]);
  });

  it('asks for an account when there are none', () => {
    render(<Stage {...base} game="osrs" accounts={[]} pick="" onPlay={noop} />);
    expect(screen.getByText('Add a Jagex account to play')).toBeTruthy();
  });
});
