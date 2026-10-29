import { useEffect, useMemo, useRef, useState } from 'react';
import { ChevronDown, ChevronUp, Copy, Eraser, FolderOpen, Pause, Play, Save, X } from 'lucide-react';
import { api, errorText } from '../api';
import { CLIENTS, isErrorLine, isStackLine } from '../format';
import type { Instance, LogSource } from '../types';

/** Lines kept in view; older ones scroll off. */
const MAX_LINES = 5000;

function lineClass(line: string): string {
  if (isErrorLine(line) || isStackLine(line)) return 'err';
  if (/\bWARN(ING)?\b/.test(line)) return 'warn';
  if (/\bDEBUG\b/.test(line)) return 'dim';
  return '';
}

/** A shown line, or a run of folded stack frames. */
type Item = { kind: 'line'; i: number; text: string } | { kind: 'fold'; i: number; count: number };

interface Props {
  instance: Instance;
  onClose: () => void;
  onOpenFolder: () => void;
  /** Java packages whose lines get highlighted (the user's own plugins). */
  packages?: string[];
  /** Open at the last error instead of the end. */
  atError?: boolean;
}

/** Follows a game's output (or RuneLite's client.log) while it runs, polling for new text every second. */
export default function LogViewer({ instance, onClose, onOpenFolder, packages = [], atError = false }: Props) {
  const [source, setSource] = useState<LogSource>('output');
  const [lines, setLines] = useState<string[]>([]);
  const [filter, setFilter] = useState('');
  const [follow, setFollow] = useState(true);
  const [wrap, setWrap] = useState(false);
  const [info, setInfo] = useState({ path: '', missing: false, running: true });
  const [error, setError] = useState('');
  const [fold, setFold] = useState(true);
  /** Folded runs the user opened, by their first line. */
  const [opened, setOpened] = useState<Set<number>>(new Set());
  /** The error line last jumped to. */
  const [cur, setCur] = useState(-1);
  const [saved, setSaved] = useState('');
  const box = useRef<HTMLDivElement>(null);
  const jumped = useRef(!atError);

  useEffect(() => {
    let alive = true;
    let offset: number | null = null;
    let partial = '';
    let timer: ReturnType<typeof setTimeout>;
    setLines([]);
    setError('');
    const poll = async () => {
      try {
        const c = await api.readLog(instance.pid, source, offset);
        if (!alive) return;
        // The file started over (a new client.log): drop what was shown.
        if (offset !== null && c.offset < offset) {
          setLines([]);
          partial = '';
        }
        offset = c.offset;
        setInfo({ path: c.path, missing: c.missing, running: c.running });
        if (c.text) {
          const parts = (partial + c.text).split('\n');
          partial = parts.pop() ?? '';
          if (parts.length) setLines((old) => old.concat(parts).slice(-MAX_LINES));
        }
        // A game that has ended won't write more; one last read is enough.
        if (c.running) timer = setTimeout(poll, 1000);
      } catch (e) {
        if (alive) setError(errorText(e));
      }
    };
    poll();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [instance.pid, source]);

  const shown = useMemo(() => {
    const f = filter.trim().toLowerCase();
    return f ? lines.filter((l) => l.toLowerCase().includes(f)) : lines;
  }, [lines, filter]);

  const mine = useMemo(() => (packages.length ? (l: string) => packages.some((p) => l.includes(p)) : () => false), [packages]);

  // Stack frames fold into one line, except the user's own (those are usually the interesting ones).
  const items = useMemo(() => {
    const out: Item[] = [];
    let run: number[] = [];
    const flush = () => {
      if (run.length && (!fold || opened.has(run[0]!) || run.length < 3)) run.forEach((i) => out.push({ kind: 'line', i, text: shown[i]! }));
      else if (run.length) {
        let hidden = 0;
        for (const i of run) {
          if (mine(shown[i]!)) {
            if (hidden) out.push({ kind: 'fold', i: run[0]!, count: hidden });
            hidden = 0;
            out.push({ kind: 'line', i, text: shown[i]! });
          } else hidden++;
        }
        if (hidden) out.push({ kind: 'fold', i: run[0]!, count: hidden });
      }
      run = [];
    };
    shown.forEach((text, i) => {
      if (isStackLine(text)) run.push(i);
      else {
        flush();
        out.push({ kind: 'line', i, text });
      }
    });
    flush();
    return out;
  }, [shown, fold, opened, mine]);

  const errors = useMemo(() => shown.flatMap((l, i) => (isErrorLine(l) ? [i] : [])), [shown]);

  const goTo = (i: number) => {
    setCur(i);
    setFollow(false);
    requestAnimationFrame(() => box.current?.querySelector(`[data-i="${i}"]`)?.scrollIntoView({ block: 'center' }));
  };
  const nextError = (dir: 1 | -1) => {
    if (!errors.length) return;
    const n = dir > 0 ? errors.find((i) => i > cur) : [...errors].reverse().find((i) => i < cur);
    goTo(n ?? (dir > 0 ? errors[0]! : errors[errors.length - 1]!));
  };

  // Opened from a crash notice: go to the last error once the log has loaded.
  useEffect(() => {
    if (jumped.current || !lines.length) return;
    jumped.current = true;
    if (errors.length) goTo(errors[errors.length - 1]!);
  }, [lines, errors]);

  const copy = () =>
    navigator.clipboard.writeText(shown.join('\n')).then(
      () => setSaved('Copied.'),
      (e) => setSaved(`Couldn't copy: ${errorText(e)}`),
    );
  const save = () =>
    api.saveLog(shown.join('\n') + '\n').then(
      (path) => setSaved(`Saved to ${path}`),
      (e) => setSaved(errorText(e)),
    );

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [shown, follow]);

  // Scrolling up pauses following; scrolling back to the bottom resumes it.
  const onScroll = () => {
    const el = box.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
    if (atBottom !== follow) setFollow(atBottom);
  };

  return (
    <div className="overlay" onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal log-modal" role="dialog" aria-modal="true" aria-label="Game log" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
        <div className="modal-head">
          <h2>
            {instance.character} · {CLIENTS[instance.client].name}
            {!info.running && <span className="badge warn" style={{ marginLeft: 8 }}>exited</span>}
          </h2>
          <button className="icon-btn" title="Close" onClick={onClose}>
            <X size={16} />
          </button>
        </div>
        <div className="log-tools">
          {instance.client === 'runelite' && (
            <div className="seg" role="tablist" aria-label="Log">
              <button role="tab" aria-selected={source === 'output'} className={source === 'output' ? 'active' : ''} onClick={() => setSource('output')}>
                Output
              </button>
              <button role="tab" aria-selected={source === 'client'} className={source === 'client' ? 'active' : ''} onClick={() => setSource('client')}>
                client.log
              </button>
            </div>
          )}
          <input className="input" style={{ flex: 1 }} placeholder="Filter…" value={filter} onChange={(e) => setFilter(e.target.value)} autoFocus />
          <label className="check" title="Wrap long lines">
            <input type="checkbox" checked={wrap} onChange={(e) => setWrap(e.target.checked)} /> Wrap
          </label>
          <button className="icon-btn" title={follow ? 'Pause following' : 'Follow new lines'} onClick={() => setFollow((f) => !f)}>
            {follow ? <Pause size={14} /> : <Play size={14} />}
          </button>
          <button className="icon-btn" title="Previous error" disabled={!errors.length} onClick={() => nextError(-1)}>
            <ChevronUp size={14} />
          </button>
          <button className="icon-btn" title="Next error" disabled={!errors.length} onClick={() => nextError(1)}>
            <ChevronDown size={14} />
          </button>
          <label className="check" title="Fold Java stack traces, keeping your own plugin's frames">
            <input type="checkbox" checked={fold} onChange={(e) => setFold(e.target.checked)} /> Fold traces
          </label>
          <button className="icon-btn" title="Copy the shown lines" disabled={!shown.length} onClick={copy}>
            <Copy size={14} />
          </button>
          <button className="icon-btn" title="Save the shown lines to the logs folder" disabled={!shown.length} onClick={save}>
            <Save size={14} />
          </button>
          <button className="icon-btn" title="Clear the view" onClick={() => setLines([])}>
            <Eraser size={14} />
          </button>
          <button className="icon-btn" title="Open the logs folder" onClick={onOpenFolder}>
            <FolderOpen size={14} />
          </button>
        </div>
        <div className={`log-view selectable ${wrap ? 'wrap' : ''}`} ref={box} onScroll={onScroll}>
          {error ? (
            <div className="log-empty err">{error}</div>
          ) : !shown.length ? (
            <div className="log-empty">
              {info.missing
                ? source === 'client'
                  ? 'RuneLite hasn’t written client.log yet.'
                  : 'No output yet.'
                : filter
                  ? 'No lines match the filter.'
                  : 'Waiting for output…'}
            </div>
          ) : (
            items.map((it, n) =>
              it.kind === 'fold' ? (
                <button key={`f${n}`} className="log-fold" onClick={() => setOpened((o) => new Set(o).add(it.i))}>
                  … {it.count} more frame{it.count === 1 ? '' : 's'}
                </button>
              ) : (
                <div key={it.i} data-i={it.i} className={`${lineClass(it.text)} ${mine(it.text) ? 'mine' : ''} ${it.i === cur ? 'cur' : ''}`}>
                  {it.text || ' '}
                </div>
              ),
            )
          )}
        </div>
        <div className="log-foot mono dim" title={info.path}>
          {shown.length !== lines.length ? `${shown.length} of ${lines.length} lines` : `${lines.length} lines`}
          {errors.length ? ` · ${errors.length} error${errors.length === 1 ? '' : 's'}` : ''} · {saved || info.path}
        </div>
      </div>
    </div>
  );
}
