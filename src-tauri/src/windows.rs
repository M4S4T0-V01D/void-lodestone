//! The games' X11 windows: naming them after their character, putting each back where it was last time, and tiling
//! them over the monitors. This only moves and names windows through the window manager; it never sends a game any
//! input. One thread owns the X connection: it looks at the windows every couple of seconds and runs the commands
//! sent to it.

use std::collections::{BTreeMap, HashSet};
use std::ffi::{c_int, c_long, c_uchar, c_ulong, CString};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use serde::Serialize;
use x11_dl::xlib::{self, Display, Window, Xlib};
use x11_dl::xrandr::Xrandr;

use crate::config::WindowSpot;

/// Smaller windows are splash screens and dialogs, not the game.
const MIN_W: u32 = 500;
const MIN_H: u32 = 300;

/// A running game whose window is looked for.
pub struct Target {
    /// The game's process group and session (it's started with setsid).
    pub pid: u32,
    /// `account id:character id`.
    pub pick: String,
    /// The character's name; empty for one that hasn't been named yet.
    pub name: String,
    /// "RuneLite", "HDOS" or "RuneScape".
    pub client: &'static str,
}

/// What the poller works on, read fresh on each look.
pub struct Job {
    pub targets: Vec<Target>,
    pub titles: bool,
    pub remember: bool,
    pub saved: BTreeMap<String, WindowSpot>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

pub enum Cmd {
    /// Tile the games over every monitor, or over the one given (an index into `monitors`).
    Tile(Option<usize>, Sender<Result<usize, String>>),
    /// Put every game back where it was saved.
    Restore(Sender<Result<usize, String>>),
    Monitors(Sender<Result<Vec<Monitor>, String>>),
}

const NO_X: &str = "Arranging windows needs an X11 display (or XWayland), and none was found.";

/// Run the window thread until the sender side is dropped. `job` reads what to work on; `record` saves the spots
/// that changed.
pub fn run(rx: Receiver<Cmd>, job: impl Fn() -> Job, record: impl Fn(Vec<(String, WindowSpot)>)) {
    let mut x: Option<X> = None;
    let mut tried = None::<Instant>;
    let mut seen = HashSet::new();
    loop {
        let cmd = match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(c) => Some(c),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let j = job();
        if x.is_none() && (cmd.is_some() || !j.targets.is_empty()) && tried.is_none_or(|t| t.elapsed() > Duration::from_secs(30))
        {
            tried = Some(Instant::now());
            x = X::open();
        }
        let Some(xc) = x.as_ref() else {
            match cmd {
                Some(Cmd::Tile(_, tx) | Cmd::Restore(tx)) => drop(tx.send(Err(NO_X.into()))),
                Some(Cmd::Monitors(tx)) => drop(tx.send(Err(NO_X.into()))),
                None => {}
            }
            continue;
        };
        xc.guard();
        let found = xc.find(&j.targets);
        match cmd {
            Some(Cmd::Tile(only, tx)) => {
                let _ = tx.send(xc.tile(&found, only));
            }
            Some(Cmd::Restore(tx)) => {
                let n = found.iter().filter_map(|(t, w)| j.saved.get(&t.pick).map(|s| xc.place(*w, s))).count();
                xc.flush();
                let _ = tx.send(Ok(n));
            }
            Some(Cmd::Monitors(tx)) => {
                let _ = tx.send(Ok(xc.monitors()));
            }
            None => {
                let changes = xc.tick(&found, &j, &mut seen);
                if !changes.is_empty() {
                    record(changes);
                }
            }
        }
        if j.targets.is_empty() {
            seen.clear();
        }
    }
}

/// The session a process belongs to, from /proc/<pid>/stat (the field after the process group).
fn session_of(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_session(&stat)
}

fn parse_session(stat: &str) -> Option<u32> {
    // The command name is in parentheses and may hold spaces or parentheses itself.
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(3)?.parse().ok()
}

/// Spread `n` windows over the monitors in order, then lay each monitor's share out in a grid as square as can be.
/// Returns the outer rectangle (frame included) of each window.
fn layout(monitors: &[Monitor], n: usize) -> Vec<(i32, i32, u32, u32)> {
    let mut out = Vec::with_capacity(n);
    if monitors.is_empty() {
        return out;
    }
    let m = monitors.len();
    for (i, mon) in monitors.iter().enumerate() {
        let k = n / m + usize::from(i < n % m);
        if k == 0 {
            continue;
        }
        let cols = (k as f64).sqrt().ceil() as usize;
        let rows = k.div_ceil(cols);
        let (cw, ch) = (mon.width / cols as u32, mon.height / rows as u32);
        for j in 0..k {
            let (c, r) = (j % cols, j / cols);
            out.push((mon.x + (c as u32 * cw) as i32, mon.y + (r as u32 * ch) as i32, cw, ch));
        }
    }
    out
}

fn intersect(m: Monitor, (x, y, w, h): (i32, i32, u32, u32)) -> Monitor {
    let (x0, y0) = (m.x.max(x), m.y.max(y));
    let x1 = (m.x + m.width as i32).min(x + w as i32);
    let y1 = (m.y + m.height as i32).min(y + h as i32);
    if x1 <= x0 || y1 <= y0 {
        return m;
    }
    Monitor { x: x0, y: y0, width: (x1 - x0) as u32, height: (y1 - y0) as u32, primary: m.primary }
}

/// Errors on this display (a window closing between listing and asking about it) are ignored; any other display's
/// go to the handler that was there before, so the toolkit's own handling is kept.
static OURS: AtomicPtr<Display> = AtomicPtr::new(std::ptr::null_mut());
static PREV: AtomicUsize = AtomicUsize::new(0);

type Handler = unsafe extern "C" fn(*mut Display, *mut xlib::XErrorEvent) -> c_int;

unsafe extern "C" fn on_error(d: *mut Display, e: *mut xlib::XErrorEvent) -> c_int {
    if d == OURS.load(Ordering::Relaxed) {
        return 0;
    }
    match PREV.load(Ordering::Relaxed) {
        0 => 0,
        // SAFETY: only ever stored from an `Option<Handler>` that was Some.
        p => unsafe { std::mem::transmute::<usize, Handler>(p)(d, e) },
    }
}

struct Atoms {
    client_list: c_ulong,
    pid: c_ulong,
    name: c_ulong,
    utf8: c_ulong,
    moveresize: c_ulong,
    state: c_ulong,
    max_v: c_ulong,
    max_h: c_ulong,
    hidden: c_ulong,
    frame: c_ulong,
    workarea: c_ulong,
    desktop: c_ulong,
}

struct X {
    xl: Xlib,
    xr: Option<Xrandr>,
    dpy: *mut Display,
    root: Window,
    a: Atoms,
}

impl Drop for X {
    fn drop(&mut self) {
        OURS.store(std::ptr::null_mut(), Ordering::Relaxed);
        unsafe { (self.xl.XCloseDisplay)(self.dpy) };
    }
}

impl X {
    fn open() -> Option<X> {
        std::env::var_os("DISPLAY").filter(|d| !d.is_empty())?;
        let xl = Xlib::open().ok()?;
        let dpy = unsafe { (xl.XOpenDisplay)(std::ptr::null()) };
        if dpy.is_null() {
            return None;
        }
        let atom = |n: &str| {
            let c = CString::new(n).unwrap();
            unsafe { (xl.XInternAtom)(dpy, c.as_ptr(), 0) }
        };
        let a = Atoms {
            client_list: atom("_NET_CLIENT_LIST"),
            pid: atom("_NET_WM_PID"),
            name: atom("_NET_WM_NAME"),
            utf8: atom("UTF8_STRING"),
            moveresize: atom("_NET_MOVERESIZE_WINDOW"),
            state: atom("_NET_WM_STATE"),
            max_v: atom("_NET_WM_STATE_MAXIMIZED_VERT"),
            max_h: atom("_NET_WM_STATE_MAXIMIZED_HORZ"),
            hidden: atom("_NET_WM_STATE_HIDDEN"),
            frame: atom("_NET_FRAME_EXTENTS"),
            workarea: atom("_NET_WORKAREA"),
            desktop: atom("_NET_CURRENT_DESKTOP"),
        };
        let root = unsafe { (xl.XDefaultRootWindow)(dpy) };
        let x = X { xr: Xrandr::open().ok(), xl, dpy, root, a };
        OURS.store(dpy, Ordering::Relaxed);
        x.guard();
        Some(x)
    }

    /// Make sure errors on this connection can't end the program; the toolkit may have set its own handler since.
    fn guard(&self) {
        let old = unsafe { (self.xl.XSetErrorHandler)(Some(on_error)) };
        if let Some(h) = old.filter(|h| *h as usize != on_error as Handler as usize) {
            PREV.store(h as usize, Ordering::Relaxed);
        }
    }

    fn flush(&self) {
        unsafe { (self.xl.XSync)(self.dpy, 0) };
    }

    /// A property's raw items: bytes for format 8, longs for format 32.
    fn prop(&self, w: Window, atom: c_ulong, want: c_ulong) -> Option<(c_int, Vec<c_ulong>, Vec<u8>)> {
        let (mut ty, mut format, mut n, mut after) = (0, 0, 0, 0);
        let mut data: *mut c_uchar = std::ptr::null_mut();
        let ok = unsafe {
            (self.xl.XGetWindowProperty)(self.dpy, w, atom, 0, 4096, 0, want, &mut ty, &mut format, &mut n, &mut after, &mut data)
        };
        if ok != 0 || data.is_null() {
            return None;
        }
        let out = match format {
            // Format 32 items come back as C longs, whatever their size.
            32 => (32, unsafe { std::slice::from_raw_parts(data as *const c_ulong, n as usize) }.to_vec(), vec![]),
            8 => (8, vec![], unsafe { std::slice::from_raw_parts(data, n as usize) }.to_vec()),
            f => (f, vec![], vec![]),
        };
        unsafe { (self.xl.XFree)(data.cast()) };
        (ty != 0).then_some(out)
    }

    fn longs(&self, w: Window, atom: c_ulong) -> Vec<c_ulong> {
        self.prop(w, atom, xlib::AnyPropertyType as c_ulong).map(|p| p.1).unwrap_or_default()
    }

    fn title(&self, w: Window) -> String {
        let p = self.prop(w, self.a.name, self.a.utf8).or_else(|| self.prop(w, xlib::XA_WM_NAME, xlib::AnyPropertyType as c_ulong));
        p.map(|p| String::from_utf8_lossy(&p.2).into_owned()).unwrap_or_default()
    }

    fn set_title(&self, w: Window, title: &str) {
        // WM_NAME is Latin-1, for window managers that don't read _NET_WM_NAME.
        let latin: Vec<u8> = title.replace('—', "-").chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
        unsafe {
            (self.xl.XChangeProperty)(self.dpy, w, self.a.name, self.a.utf8, 8, xlib::PropModeReplace, title.as_ptr(), title.len() as c_int);
            (self.xl.XChangeProperty)(self.dpy, w, xlib::XA_WM_NAME, xlib::XA_STRING, 8, xlib::PropModeReplace, latin.as_ptr(), latin.len() as c_int);
        }
    }

    /// Where the window's inside is on the screen, its size, and whether it's on view.
    fn geometry(&self, w: Window) -> Option<(i32, i32, u32, u32, bool)> {
        let mut at: xlib::XWindowAttributes = unsafe { std::mem::zeroed() };
        if unsafe { (self.xl.XGetWindowAttributes)(self.dpy, w, &mut at) } == 0 {
            return None;
        }
        let (mut x, mut y, mut child) = (0, 0, 0);
        if unsafe { (self.xl.XTranslateCoordinates)(self.dpy, w, self.root, 0, 0, &mut x, &mut y, &mut child) } == 0 {
            return None;
        }
        Some((x, y, at.width.max(0) as u32, at.height.max(0) as u32, at.map_state == xlib::IsViewable))
    }

    fn client_message(&self, w: Window, kind: c_ulong, data: [c_long; 5]) {
        let mut ev = xlib::XClientMessageEvent {
            type_: xlib::ClientMessage,
            serial: 0,
            send_event: 1,
            display: self.dpy,
            window: w,
            message_type: kind,
            format: 32,
            data: xlib::ClientMessageData::new(),
        };
        ev.data.as_longs_mut()[..5].copy_from_slice(&data);
        let mut ev = xlib::XEvent::from(ev);
        unsafe {
            (self.xl.XSendEvent)(self.dpy, self.root, 0, xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask, &mut ev)
        };
    }

    fn maximize(&self, w: Window, on: bool) {
        // Source 2: a pager or tool acting for the user, which window managers honour.
        self.client_message(w, self.a.state, [on as c_long, self.a.max_v as c_long, self.a.max_h as c_long, 2, 0]);
    }

    /// Move and size the window's inside (StaticGravity), unmaximizing it first.
    fn move_to(&self, w: Window, x: i32, y: i32, width: u32, height: u32) {
        self.maximize(w, false);
        let flags = 10 | (0xf << 8) | (2 << 12);
        self.client_message(w, self.a.moveresize, [flags, x as c_long, y as c_long, width as c_long, height as c_long]);
    }

    fn place(&self, w: Window, s: &WindowSpot) {
        self.move_to(w, s.x, s.y, s.width.max(MIN_W), s.height.max(MIN_H));
        if s.maximized {
            self.maximize(w, true);
        }
    }

    /// Each target's game window: the biggest one its process session owns, or failing that one titled with its name.
    fn find<'a>(&self, targets: &'a [Target]) -> Vec<(&'a Target, Window)> {
        if targets.is_empty() {
            return vec![];
        }
        let mut best: Vec<Option<(Window, u64)>> = vec![None; targets.len()];
        for w in self.longs(self.root, self.a.client_list) {
            let Some((_, _, width, height, _)) = self.geometry(w) else { continue };
            if width < MIN_W || height < MIN_H {
                continue;
            }
            let pid = self.longs(w, self.a.pid).first().map(|p| *p as u32);
            let session = pid.and_then(session_of);
            let hit = targets.iter().position(|t| pid == Some(t.pid) || session == Some(t.pid)).or_else(|| {
                if pid.is_some() {
                    return None;
                }
                let title = self.title(w);
                targets.iter().position(|t| !t.name.is_empty() && title.contains(&t.name))
            });
            if let Some(i) = hit {
                let area = width as u64 * height as u64;
                if best[i].is_none_or(|(_, a)| area > a) {
                    best[i] = Some((w, area));
                }
            }
        }
        targets.iter().zip(best).filter_map(|(t, b)| b.map(|(w, _)| (t, w))).collect()
    }

    /// Name the windows, put new ones where they were saved, and note where the others are now.
    fn tick(&self, found: &[(&Target, Window)], j: &Job, seen: &mut HashSet<Window>) -> Vec<(String, WindowSpot)> {
        let mut changes = vec![];
        for (t, w) in found {
            if j.titles && !t.name.is_empty() {
                let want = format!("{} — {}", t.name, t.client);
                if self.title(*w) != want {
                    self.set_title(*w, &want);
                }
            }
            let new = seen.insert(*w);
            if !j.remember {
                continue;
            }
            if new {
                if let Some(s) = j.saved.get(&t.pick) {
                    self.place(*w, s);
                    continue;
                }
            }
            let Some((x, y, width, height, viewable)) = self.geometry(*w) else { continue };
            let state = self.longs(*w, self.a.state);
            if !viewable || state.contains(&self.a.hidden) {
                continue;
            }
            let maximized = state.contains(&self.a.max_v) && state.contains(&self.a.max_h);
            let spot = WindowSpot { x, y, width, height, maximized };
            if j.saved.get(&t.pick) != Some(&spot) {
                changes.push((t.pick.clone(), spot));
            }
        }
        self.flush();
        changes
    }

    /// The monitors left to right, less panels and docks.
    fn monitors(&self) -> Vec<Monitor> {
        let mut out = vec![];
        if let Some(xr) = &self.xr {
            let mut n = 0;
            let list = unsafe { (xr.XRRGetMonitors)(self.dpy, self.root, 1, &mut n) };
            if !list.is_null() {
                for m in unsafe { std::slice::from_raw_parts(list, n.max(0) as usize) } {
                    if m.width > 0 && m.height > 0 {
                        out.push(Monitor { x: m.x, y: m.y, width: m.width as u32, height: m.height as u32, primary: m.primary != 0 });
                    }
                }
                unsafe { (xr.XRRFreeMonitors)(list) };
            }
        }
        if out.is_empty() {
            if let Some((_, _, width, height, _)) = self.geometry(self.root) {
                out.push(Monitor { x: 0, y: 0, width, height, primary: true });
            }
        }
        let area = self.longs(self.root, self.a.workarea);
        let desk = self.longs(self.root, self.a.desktop).first().copied().unwrap_or(0) as usize;
        if let Some(r) = area.get(desk * 4..desk * 4 + 4) {
            out = out.into_iter().map(|m| intersect(m, (r[0] as i32, r[1] as i32, r[2] as u32, r[3] as u32))).collect();
        }
        out.sort_by_key(|m| (m.x, m.y));
        out
    }

    fn tile(&self, found: &[(&Target, Window)], only: Option<usize>) -> Result<usize, String> {
        if found.is_empty() {
            return Err("No game windows were found to arrange.".into());
        }
        let mut mons = self.monitors();
        if let Some(i) = only {
            mons = vec![*mons.get(i).ok_or("That monitor isn't connected any more.")?];
        }
        for ((_, w), (x, y, cw, ch)) in found.iter().zip(layout(&mons, found.len())) {
            let f = self.longs(*w, self.a.frame);
            let [l, r, t, b] = [0, 1, 2, 3].map(|i| f.get(i).copied().unwrap_or(0) as u32);
            self.move_to(*w, x + l as i32, y + t as i32, cw.saturating_sub(l + r), ch.saturating_sub(t + b));
        }
        self.flush();
        Ok(found.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_session() {
        assert_eq!(parse_session("123 (java (x) y) S 1 120 120 0 -1"), Some(120));
        assert_eq!(parse_session("junk"), None);
        assert_eq!(session_of(std::process::id()), Some(unsafe { libc::getsid(0) } as u32));
    }

    #[test]
    fn tiles_over_monitors() {
        let a = Monitor { x: 0, y: 0, width: 1920, height: 1080, primary: true };
        let b = Monitor { x: 1920, y: 0, width: 2560, height: 1440, primary: false };
        assert_eq!(layout(&[a], 1), [(0, 0, 1920, 1080)]);
        assert_eq!(layout(&[a], 2), [(0, 0, 960, 1080), (960, 0, 960, 1080)]);
        assert_eq!(layout(&[a], 3).len(), 3);
        assert_eq!(layout(&[a], 4)[3], (960, 540, 960, 540));
        assert_eq!(layout(&[a, b], 3), [(0, 0, 960, 1080), (960, 0, 960, 1080), (1920, 0, 2560, 1440)]);
        assert_eq!(layout(&[a, b], 1), [(0, 0, 1920, 1080)]);
        assert!(layout(&[], 2).is_empty());
    }

    /// Needs a display: `cargo test -- --ignored sees_the_display --nocapture`.
    #[test]
    #[ignore]
    fn sees_the_display() {
        let x = X::open().expect("no X display");
        x.guard();
        println!("monitors {:?}", x.monitors());
        let wins = x.longs(x.root, x.a.client_list);
        for w in &wins {
            println!("{w:#x} {:?} pid {:?} {:?}", x.geometry(*w), x.longs(*w, x.a.pid).first(), x.title(*w));
        }
        assert!(x.geometry(0x1).is_none(), "a bad window is an ignored error, not the end");
    }

    #[test]
    fn keeps_monitors_inside_the_work_area() {
        let a = Monitor { x: 0, y: 0, width: 1920, height: 1080, primary: true };
        assert_eq!(intersect(a, (0, 32, 3840, 1048)), Monitor { x: 0, y: 32, width: 1920, height: 1048, primary: true });
        assert_eq!(intersect(a, (5000, 0, 10, 10)), a, "an area off this monitor leaves it as it is");
    }
}
