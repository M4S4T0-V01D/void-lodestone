import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import Accounts from './Accounts';
import CharacterModal, { EMPTY_INFO, parseTags } from './CharacterModal';
import Instances from './Instances';
import { newsDate } from './NewsModal';
import Stage from './Stage';
import type { Account, Instance } from '../types';

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

  it('hides the logo and title but keeps the heading', () => {
    const { container } = render(<Stage {...base} game="osrs" accounts={accounts} pick="a:1" onPlay={noop} hideHero />);
    expect(container.querySelector('.hero')).toBeNull();
    expect(screen.getByRole('heading', { level: 1 })).toBeTruthy();
  });

  it('places the background picture', () => {
    const { container } = render(<Stage {...base} game="osrs" accounts={accounts} pick="a:1" onPlay={noop} background="blob:x" backgroundPosition="top" />);
    expect((container.querySelector('.stage-bg') as HTMLElement).style.backgroundPosition).toBe('center top');
  });

  it('shows tags and the next goal, and opens the notes', () => {
    const onNotes = vi.fn();
    const info = { ...EMPTY_INFO, tags: ['ironman'], goals: [{ text: 'Dragon Slayer', done: true }, { text: '99 Fishing', done: false }] };
    render(<Stage {...base} game="osrs" accounts={accounts} pick="a:1" onPlay={noop} info={info} onNotes={onNotes} />);
    expect(screen.getByText('ironman')).toBeTruthy();
    expect(screen.getByText('Next: 99 Fishing')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Notes/ }));
    expect(onNotes).toHaveBeenCalled();
  });
});

describe('parseTags', () => {
  it('trims, drops blanks and repeats', () => {
    expect(parseTags(' ironman, skiller,, Ironman ,pker ')).toEqual(['ironman', 'skiller', 'pker']);
    expect(parseTags('')).toEqual([]);
  });
});

describe('newsDate', () => {
  it('shortens feed dates and keeps what it cannot read', () => {
    expect(newsDate('Wed, 11 Nov 2026 11:00:00 GMT')).toMatch(/2026/);
    expect(newsDate('soon')).toBe('soon');
  });
});

describe('CharacterModal', () => {
  it('saves tags, colour and a typed goal', () => {
    const onSave = vi.fn();
    render(
      <CharacterModal name="Zezima" info={EMPTY_INFO} profiles={[]} ownProfiles onSave={onSave} onForgetWindow={noop} onClose={noop} />,
    );
    fireEvent.change(screen.getByLabelText('Tags'), { target: { value: 'ironman, skiller' } });
    fireEvent.click(screen.getByTitle('#5fb36b'));
    fireEvent.change(screen.getByPlaceholderText(/Quest X next/), { target: { value: 'Quest cape' } });
    fireEvent.click(screen.getByRole('button', { name: /Save/ }));
    expect(onSave).toHaveBeenCalledWith({ ...EMPTY_INFO, tags: ['ironman', 'skiller'], color: '#5fb36b', goals: [{ text: 'Quest cape', done: false }] });
  });

  it('shows and forgets the remembered window', () => {
    const onForget = vi.fn();
    render(
      <CharacterModal
        name="Zezima"
        info={EMPTY_INFO}
        profiles={[]}
        ownProfiles={false}
        window={{ x: 10, y: 20, width: 800, height: 600, maximized: false }}
        onSave={noop}
        onForgetWindow={onForget}
        onClose={noop}
      />,
    );
    expect(screen.getByText(/800×600 at 10, 20/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Forget/ }));
    expect(onForget).toHaveBeenCalled();
  });
});

describe('Instances', () => {
  const inst: Instance = { pid: 1, client: 'runelite', accountId: 'a', accountLabel: 'Main', character: 'Zezima', startedAt: 0, log: '', characterId: '1' };

  it('tiles over the chosen monitor and restores', () => {
    const onTile = vi.fn();
    const onRestore = vi.fn();
    const monitors = [
      { x: 0, y: 0, width: 1920, height: 1080, primary: true },
      { x: 1920, y: 0, width: 2560, height: 1440, primary: false },
    ];
    render(
      <Instances instances={[inst]} onStop={noop} onStopAll={noop} onRestart={noop} onLogs={noop} onLog={noop} onTile={onTile} onRestoreWindows={onRestore} monitors={monitors} />,
    );
    fireEvent.click(screen.getByRole('button', { name: /Tile/ }));
    expect(onTile).toHaveBeenLastCalledWith(null);
    fireEvent.change(screen.getByLabelText('Tile over'), { target: { value: '1' } });
    fireEvent.click(screen.getByRole('button', { name: /Tile/ }));
    expect(onTile).toHaveBeenLastCalledWith(1);
    fireEvent.click(screen.getByRole('button', { name: /Restore/ }));
    expect(onRestore).toHaveBeenCalled();
  });

  it('hides the window tools with no games', () => {
    render(<Instances instances={[]} onStop={noop} onStopAll={noop} onRestart={noop} onLogs={noop} onLog={noop} onTile={noop} />);
    expect(screen.queryByRole('button', { name: /Tile/ })).toBeNull();
  });
});
