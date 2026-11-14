//! VOID Lodestone: a launcher for RuneLite, HDOS and RuneScape 3 with any number of Jagex accounts.
//! No telemetry: the only hosts contacted are Jagex's (sign-in, sessions, RS3 download), GitHub (RuneLite) and
//! HDOS's CDN, plus GitHub for Lodestone's own releases (which can be turned off).

mod backup;
mod clients;
mod config;
mod gradle;
mod jagex;
mod launch;
mod login;
mod news;
mod notify;
mod playtime;
mod profiles;
mod secrets;
mod tray;
mod update;
mod windows;

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

use clients::Client;
use config::{now, Account, Settings, Store};
use jagex::{ApiError, Character};

pub struct App {
    store: Mutex<Store>,
    secrets: tokio::sync::OnceCell<secrets::Backend>,
    http: reqwest::Client,
    login_open: Mutex<bool>,
    /// Accounts whose session Jagex no longer accepts.
    expired: Mutex<HashSet<String>>,
    /// The last check of each account's session.
    health: Mutex<HashMap<String, Health>>,
    /// Whether each account has a saved session, so building the state doesn't ask the keyring every time.
    has_session: Mutex<HashMap<String, bool>>,
    instances: Arc<Mutex<Vec<Instance>>>,
    /// Games that have exited, newest last, so their logs can still be read.
    ended: Arc<Mutex<Vec<Instance>>>,
    /// One download at a time, so a launch and an update never write the same file.
    downloads: tokio::sync::Mutex<()>,
    /// Games being stopped from here, so their ending isn't taken for a crash.
    stopping: Mutex<HashSet<u32>>,
    /// The running plugin build's process group (0 between projects); None when no build runs.
    build: Mutex<Option<u32>>,
    /// Whether there's a tray icon to close to.
    tray: AtomicBool,
    /// The newest Lodestone release seen on GitHub.
    release: Mutex<Option<update::Release>>,
    /// An update is being downloaded or installed.
    updating: AtomicBool,
    /// Commands for the thread that names, places and tiles the game windows.
    windows: Mutex<std::sync::mpsc::Sender<windows::Cmd>>,
}

impl App {
    async fn secrets(&self) -> &secrets::Backend {
        self.secrets.get_or_init(secrets::Backend::open).await
    }

    /// Whether the account has a saved session; asks the keyring only the first time.
    async fn has_session(&self, id: &str) -> bool {
        if let Some(&yes) = self.has_session.lock().unwrap().get(id) {
            return yes;
        }
        let yes = self.secrets().await.get(id).await.is_some();
        self.has_session.lock().unwrap().insert(id.to_string(), yes);
        yes
    }

    fn save(&self, store: &Store) -> Result<(), String> {
        store.save_to(&Store::path()).map_err(|e| format!("couldn't save settings: {e}"))
    }

    fn set_expired(&self, id: &str, yes: bool) {
        let mut e = self.expired.lock().unwrap();
        if yes {
            e.insert(id.to_string());
        } else {
            e.remove(id);
        }
    }

    fn set_health(&self, id: &str, state: &'static str) {
        self.health.lock().unwrap().insert(id.to_string(), Health { state, checked_at: now() });
    }

    fn find_instance(&self, pid: u32) -> Option<(Instance, bool)> {
        let running = self.instances.lock().unwrap().iter().find(|i| i.pid == pid).cloned();
        running.map(|i| (i, true)).or_else(|| {
            self.ended.lock().unwrap().iter().rev().find(|i| i.pid == pid).cloned().map(|i| (i, false))
        })
    }
}

/// ok: Jagex accepted the session; expired: it didn't (or none is saved); offline: Jagex couldn't be reached.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    state: &'static str,
    checked_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pid: u32,
    client: Client,
    account_id: String,
    account_label: String,
    character_id: String,
    character: String,
    started_at: u64,
    log: String,
    /// Started with the custom jar rather than the downloaded client.
    custom_jar: bool,
    #[serde(skip)]
    jar_path: String,
    /// When the jar or a sideloaded plugin last changed, as of the launch.
    #[serde(skip)]
    jar_stamp: u64,
    /// The jar or a sideloaded plugin changed since this game started.
    rebuilt: bool,
    /// The JDWP port a debugger can attach to.
    debug_port: Option<u16>,
    /// For games that have ended:
    ended_at: u64,
    exit_code: Option<i32>,
    /// It exited with an error, or within a minute, without being stopped from here.
    crashed: bool,
    /// The last error line in its output, when it crashed.
    crash_line: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountView {
    id: String,
    label: String,
    characters: Vec<Character>,
    added_at: u64,
    signed_in_at: u64,
    signed_in: bool,
    health: Option<Health>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateView {
    version: &'static str,
    flatpak: bool,
    secret_backend: &'static str,
    settings: Settings,
    accounts: Vec<AccountView>,
    clients: Vec<clients::Status>,
    instances: Vec<Instance>,
    login_open: bool,
    data_dir: String,
    /// The Java packages of the user's plugin projects, highlighted in logs.
    plugin_packages: Vec<String>,
    tray: bool,
    /// A newer release, once one has been found.
    update: Option<update::Release>,
    /// Notes, goals, tags, colour and profile per pick.
    characters: std::collections::BTreeMap<String, config::CharacterInfo>,
    /// Where each pick's game window was last seen.
    windows: std::collections::BTreeMap<String, config::WindowSpot>,
}

async fn view(st: &App) -> StateView {
    let store = st.store.lock().unwrap().clone();
    let backend = st.secrets().await;
    let mut accounts = Vec::new();
    for a in store.accounts {
        let expired = st.expired.lock().unwrap().contains(&a.id);
        let signed_in = !expired && st.has_session(&a.id).await;
        let health = st.health.lock().unwrap().get(&a.id).cloned();
        accounts.push(AccountView {
            id: a.id,
            label: a.label,
            characters: a.characters,
            added_at: a.added_at,
            signed_in_at: a.signed_in_at,
            signed_in,
            health,
        });
    }
    let mut plugin_packages: Vec<String> = gradle::projects(&store.settings).iter().flat_map(|p| gradle::packages(p)).collect();
    plugin_packages.dedup();
    StateView {
        version: env!("CARGO_PKG_VERSION"),
        flatpak: config::in_flatpak(),
        secret_backend: backend.name(),
        settings: store.settings,
        accounts,
        clients: Client::ALL.iter().map(|c| clients::status(*c)).collect(),
        instances: st.instances.lock().unwrap().clone(),
        login_open: *st.login_open.lock().unwrap(),
        data_dir: config::data_dir().to_string_lossy().into(),
        plugin_packages,
        tray: st.tray.load(Ordering::Relaxed),
        update: st.release.lock().unwrap().clone().filter(|r| r.newer),
        characters: store.characters,
        windows: store.windows,
    }
}

#[tauri::command]
async fn get_state(st: State<'_, App>) -> Result<StateView, String> {
    Ok(view(&st).await)
}

/// Save a character's notes, goals, tags, colour and profile; an empty one is removed.
#[tauri::command]
fn save_character(st: State<'_, App>, pick: String, info: config::CharacterInfo) -> Result<(), String> {
    let mut store = st.store.lock().unwrap();
    let info = config::CharacterInfo {
        tags: info.tags.into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
        goals: info.goals.into_iter().filter(|g| !g.text.trim().is_empty()).collect(),
        profile: info.profile.trim().to_string(),
        ..info
    };
    if !info.profile.is_empty() && !profiles::valid_name(&info.profile) {
        return Err(format!("\"{}\" can't be used as a RuneLite profile name.", info.profile));
    }
    if info == config::CharacterInfo::default() {
        store.characters.remove(&pick);
    } else {
        store.characters.insert(pick, info);
    }
    st.save(&store)
}

#[tauri::command]
fn runelite_profiles(st: State<'_, App>) -> Result<Vec<profiles::Profile>, String> {
    let settings = st.store.lock().unwrap().settings.clone();
    profiles::list(&profiles::dir(&settings))
}

/// Copy one RuneLite profile's settings over others. A running RuneLite would save its own over them, so not then.
#[tauri::command]
async fn share_profile(st: State<'_, App>, from: String, to: Vec<String>) -> Result<usize, String> {
    if st.instances.lock().unwrap().iter().any(|i| i.client == Client::Runelite) {
        return Err("Close RuneLite first: a running RuneLite saves its own settings over the copies.".into());
    }
    let settings = st.store.lock().unwrap().settings.clone();
    let dir = profiles::dir(&settings);
    tauri::async_runtime::spawn_blocking(move || profiles::share(&dir, &from, &to)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn save_settings(app: AppHandle, st: State<'_, App>, settings: Settings) -> Result<(), String> {
    {
        let mut store = st.store.lock().unwrap();
        store.settings = settings;
        st.save(&store)?;
    }
    tray::refresh(&app);
    Ok(())
}

async fn sign_in(app: AppHandle, st: &App, existing: Option<String>, mode: login::Mode) -> Result<StateView, String> {
    {
        let mut open = st.login_open.lock().unwrap();
        if *open {
            if let Some(w) = app.get_webview_window(login::WINDOW) {
                let _ = w.set_focus();
            }
            return Err("A sign-in window is already open.".into());
        }
        *open = true;
    }
    let id = existing.clone().unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    let res = login::run(app.clone(), st.http.clone(), id.clone(), mode).await;
    *st.login_open.lock().unwrap() = false;
    let out = res?;

    let target = {
        let mut store = st.store.lock().unwrap();
        let known = |a: &Account| !a.subject.is_empty() && a.subject == out.subject && out.subject != "unknown";
        let target = match &existing {
            Some(id) => {
                let a = store.accounts.iter().find(|a| &a.id == id).ok_or("that account was removed")?;
                if !a.subject.is_empty() && a.subject != out.subject {
                    return Err("That's a different Jagex account from this one. Use \"Add account\" for it.".into());
                }
                id.clone()
            }
            None => match store.accounts.iter().find(|a| known(a)) {
                // Signed in to an account that's already here: refresh it rather than adding a copy.
                Some(a) => {
                    let _ = std::fs::remove_dir_all(login::cookie_dir(&id));
                    a.id.clone()
                }
                None => {
                    let n = store.accounts.len() + 1;
                    let label = out
                        .characters
                        .iter()
                        .find(|c| !c.display_name.is_empty())
                        .map(|c| c.display_name.clone())
                        .unwrap_or_else(|| format!("Jagex account {n}"));
                    store.accounts.push(Account {
                        id: id.clone(),
                        label,
                        subject: String::new(),
                        characters: Vec::new(),
                        added_at: now(),
                        signed_in_at: 0,
                    });
                    id.clone()
                }
            },
        };
        let a = store.accounts.iter_mut().find(|a| a.id == target).unwrap();
        a.subject = out.subject.clone();
        a.characters = out.characters.clone();
        a.signed_in_at = now();
        st.save(&store)?;
        target
    };
    st.secrets().await.set(&target, &out.session_id).await?;
    st.has_session.lock().unwrap().insert(target.clone(), true);
    st.set_expired(&target, false);
    st.set_health(&target, "ok");
    Ok(view(st).await)
}

#[tauri::command]
async fn add_account(app: AppHandle, st: State<'_, App>, mode: login::Mode) -> Result<StateView, String> {
    sign_in(app, &st, None, mode).await
}

#[tauri::command]
async fn reauth_account(app: AppHandle, st: State<'_, App>, id: String, mode: login::Mode) -> Result<StateView, String> {
    sign_in(app, &st, Some(id), mode).await
}

#[tauri::command]
fn submit_login_url(url: String) -> Result<(), String> {
    login::submit(&url)
}

#[tauri::command]
fn cancel_login(app: AppHandle) {
    login::cancel(&app);
}

#[tauri::command]
async fn remove_account(st: State<'_, App>, id: String) -> Result<StateView, String> {
    {
        let mut store = st.store.lock().unwrap();
        store.accounts.retain(|a| a.id != id);
        st.save(&store)?;
    }
    st.secrets().await.delete(&id).await;
    st.has_session.lock().unwrap().remove(&id);
    st.set_expired(&id, false);
    st.health.lock().unwrap().remove(&id);
    if !id.is_empty() && !id.contains('/') && !id.contains("..") {
        let _ = std::fs::remove_dir_all(login::cookie_dir(&id));
    }
    Ok(view(&st).await)
}

#[tauri::command]
fn rename_account(st: State<'_, App>, id: String, label: String) -> Result<(), String> {
    let label: String = label.trim().chars().take(40).collect();
    if label.is_empty() {
        return Err("The name can't be empty.".into());
    }
    let mut store = st.store.lock().unwrap();
    let a = store.accounts.iter_mut().find(|a| a.id == id).ok_or("no such account")?;
    a.label = label;
    st.save(&store)
}

#[tauri::command]
fn move_account(st: State<'_, App>, id: String, to: usize) -> Result<(), String> {
    let mut store = st.store.lock().unwrap();
    let from = store.accounts.iter().position(|a| a.id == id).ok_or("no such account")?;
    let a = store.accounts.remove(from);
    let to = to.min(store.accounts.len());
    store.accounts.insert(to, a);
    st.save(&store)
}

/// Re-read each account's characters with its saved session, which also tells which sessions have expired.
async fn check_account(st: &App, id: &str, wait: Duration) -> Result<Option<String>, String> {
    let session = st.secrets().await.get(id).await;
    st.has_session.lock().unwrap().insert(id.to_string(), session.is_some());
    let Some(session) = session else {
        st.set_expired(id, true);
        st.set_health(id, "expired");
        return Ok(None);
    };
    match tokio::time::timeout(wait, jagex::characters(&st.http, &session)).await {
        Ok(Ok(chars)) => {
            st.set_expired(id, false);
            st.set_health(id, "ok");
            let mut store = st.store.lock().unwrap();
            if let Some(a) = store.accounts.iter_mut().find(|a| a.id == id) {
                if a.characters != chars {
                    a.characters = chars;
                    st.save(&store)?;
                }
            }
            Ok(Some(session))
        }
        Ok(Err(ApiError::Expired)) => {
            st.set_expired(id, true);
            st.set_health(id, "expired");
            Ok(None)
        }
        // Offline or Jagex is slow: keep what we have and let the game client try.
        _ => {
            st.set_health(id, "offline");
            Ok(Some(session))
        }
    }
}

/// Check every account's session. Returns the labels of accounts that were fine before and have now expired.
async fn check_all(st: &App) -> Result<Vec<String>, String> {
    let accounts: Vec<(String, String)> =
        st.store.lock().unwrap().accounts.iter().map(|a| (a.id.clone(), a.label.clone())).collect();
    let mut lost = Vec::new();
    for (id, label) in accounts {
        let was_ok = !st.expired.lock().unwrap().contains(&id);
        if check_account(st, &id, Duration::from_secs(10)).await?.is_none() && was_ok {
            lost.push(label);
        }
    }
    Ok(lost)
}

#[tauri::command]
async fn refresh_accounts(st: State<'_, App>) -> Result<StateView, String> {
    check_all(&st).await?;
    Ok(view(&st).await)
}

fn progress_emitter(app: &AppHandle) -> impl Fn(clients::Progress) + Send + Sync {
    let app = app.clone();
    move |p| {
        let _ = app.emit("client-progress", p);
    }
}

#[tauri::command]
async fn update_clients(
    app: AppHandle,
    st: State<'_, App>,
    only: Option<Client>,
    force: bool,
) -> Result<Vec<clients::Status>, String> {
    let _guard = st.downloads.lock().await;
    let emit = progress_emitter(&app);
    let mut errors = Vec::new();
    for c in Client::ALL {
        if only.is_some_and(|o| o != c) {
            continue;
        }
        if let Err(e) = clients::ensure(&st.http, c, force, &emit).await {
            errors.push(e);
        }
    }
    if !errors.is_empty() && only.is_some() {
        return Err(errors.join("\n"));
    }
    Ok(Client::ALL.iter().map(|c| clients::status(*c)).collect())
}

/// Go back to the client version the last update replaced, and keep it until updates resume.
#[tauri::command]
async fn rollback_client(st: State<'_, App>, client: Client) -> Result<Vec<clients::Status>, String> {
    if st.instances.lock().unwrap().iter().any(|i| i.client == client) {
        return Err(format!("Close {} first.", client.name()));
    }
    let _guard = st.downloads.lock().await;
    clients::roll_back(client)?;
    Ok(Client::ALL.iter().map(|c| clients::status(*c)).collect())
}

/// Let a rolled-back client update again, and update it now.
#[tauri::command]
async fn resume_updates(app: AppHandle, st: State<'_, App>, client: Client) -> Result<Vec<clients::Status>, String> {
    let _guard = st.downloads.lock().await;
    clients::resume(client);
    clients::ensure(&st.http, client, true, &progress_emitter(&app)).await?;
    Ok(Client::ALL.iter().map(|c| clients::status(*c)).collect())
}

/// Feed the window thread the running games and save the window spots it reports.
fn window_thread(app: AppHandle, rx: std::sync::mpsc::Receiver<windows::Cmd>) {
    let job = || {
        let st = app.state::<App>();
        let running = st.instances.lock().unwrap().clone();
        let store = st.store.lock().unwrap();
        let targets = running
            .iter()
            .map(|i| windows::Target {
                pid: i.pid,
                pick: format!("{}:{}", i.account_id, i.character_id),
                name: store
                    .character(&i.account_id, &i.character_id)
                    .map(|(_, c)| c.display_name.trim().to_string())
                    .unwrap_or_default(),
                client: if i.client == Client::Rs3 { "RuneScape" } else { i.client.name() },
            })
            .collect();
        windows::Job {
            targets,
            titles: store.settings.window_titles,
            remember: store.settings.remember_windows,
            saved: store.windows.clone(),
        }
    };
    let record = |changes: Vec<(String, config::WindowSpot)>| {
        let st = app.state::<App>();
        let mut store = st.store.lock().unwrap();
        store.windows.extend(changes);
        let _ = st.save(&store);
    };
    windows::run(rx, job, record);
}

/// Ask the window thread something and wait for its answer off the async runtime.
async fn ask_windows<T: Send + 'static>(
    st: &App,
    cmd: impl FnOnce(std::sync::mpsc::Sender<Result<T, String>>) -> windows::Cmd,
) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    st.windows.lock().unwrap().send(cmd(tx)).map_err(|_| "The window helper has stopped.".to_string())?;
    tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(Duration::from_secs(10)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|_| "The window helper didn't answer.".to_string())?
}

/// Tile the game windows over every monitor, or over one (an index from `monitors`). Returns how many moved.
#[tauri::command]
async fn tile_windows(st: State<'_, App>, monitor: Option<usize>) -> Result<usize, String> {
    ask_windows(&st, |tx| windows::Cmd::Tile(monitor, tx)).await
}

/// Put the game windows back where they were saved.
#[tauri::command]
async fn restore_windows(st: State<'_, App>) -> Result<usize, String> {
    ask_windows(&st, windows::Cmd::Restore).await
}

#[tauri::command]
async fn monitors(st: State<'_, App>) -> Result<Vec<windows::Monitor>, String> {
    ask_windows(&st, windows::Cmd::Monitors).await
}

/// The latest news for "osrs" or "rs3".
#[tauri::command]
async fn news(st: State<'_, App>, game: String) -> Result<Vec<news::Item>, String> {
    news::feed(&st.http, &game).await
}

#[tauri::command]
async fn server_status(st: State<'_, App>) -> Result<news::Status, String> {
    Ok(news::status(&st.http).await)
}

/// Open a news link in the browser; only Jagex's own pages.
#[tauri::command]
fn open_news(url: String) -> Result<(), String> {
    if !news::may_open(&url) {
        return Err("That link doesn't go to a Jagex news page.".into());
    }
    std::process::Command::new("xdg-open")
        .arg(&url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("couldn't open the link: {e}"))
}

fn runelite_running(st: &App) -> bool {
    st.instances.lock().unwrap().iter().any(|i| i.client == Client::Runelite)
}

#[tauri::command]
fn backups() -> Vec<backup::Backup> {
    backup::list()
}

/// Back up settings, groups, character notes, play time and RuneLite's profiles. Returns the new file's name.
#[tauri::command]
async fn make_backup(st: State<'_, App>) -> Result<String, String> {
    let store = st.store.lock().unwrap().clone();
    let rl = launch::runelite_dir(&store.settings);
    tauri::async_runtime::spawn_blocking(move || backup::create(&store, &rl)).await.map_err(|e| e.to_string())?
}

/// Put a backup back: Lodestone's part (settings, groups, notes, window spots, play time), RuneLite's profiles, or both.
#[tauri::command]
async fn restore_backup(app: AppHandle, st: State<'_, App>, name: String, lodestone: bool, runelite: bool) -> Result<StateView, String> {
    if runelite && runelite_running(&st) {
        return Err("Close RuneLite first: a running RuneLite saves its own settings over the restored ones.".into());
    }
    let path = backup::path_of(&name)?;
    let c = tauri::async_runtime::spawn_blocking(move || backup::read(&path)).await.map_err(|e| e.to_string())??;
    if lodestone {
        if let Some(from) = c.store.clone() {
            let mut store = st.store.lock().unwrap();
            let ids = backup::merge(&mut store, from);
            st.save(&store)?;
            if let Some(book) = &c.playtime {
                let book: playtime::Book = book.iter().map(|(k, v)| (backup::remap(&ids, k), v.clone())).collect();
                let p = playtime::path();
                let tmp = p.with_extension("json.tmp");
                std::fs::write(&tmp, serde_json::to_vec(&book).unwrap())
                    .and_then(|_| std::fs::rename(&tmp, &p))
                    .map_err(|e| format!("couldn't restore play time: {e}"))?;
            }
        }
        st.has_session.lock().unwrap().clear();
    }
    if runelite {
        let rl = launch::runelite_dir(&st.store.lock().unwrap().settings);
        tauri::async_runtime::spawn_blocking(move || backup::put_runelite(&rl, &c)).await.map_err(|e| e.to_string())??;
    }
    tray::refresh(&app);
    Ok(view(&st).await)
}

/// Bolt's RuneLite folders found on this computer.
#[tauri::command]
fn bolt_folders() -> Vec<String> {
    backup::bolt_folders().iter().map(|p| p.display().to_string()).collect()
}

/// Copy RuneLite's profiles from Bolt (or any .runelite folder) into the one Lodestone's RuneLite uses. Only reads
/// profiles2 and settings.properties; never Bolt's saved sign-in. Returns how many profiles came over.
#[tauri::command]
async fn import_bolt(st: State<'_, App>, folder: String) -> Result<usize, String> {
    if runelite_running(&st) {
        return Err("Close RuneLite first: a running RuneLite saves its own settings over the copied ones.".into());
    }
    let from = std::path::PathBuf::from(launch::expand_home(folder.trim()));
    let Some(from) = backup::find_runelite(&from) else {
        return Err("No RuneLite profiles there. Point at Bolt's bolt-launcher folder or the .runelite folder in it.".into());
    };
    let to = launch::runelite_dir(&st.store.lock().unwrap().settings);
    if from.canonicalize().ok() == to.canonicalize().ok() {
        return Err("That's the folder Lodestone's RuneLite already uses.".into());
    }
    tauri::async_runtime::spawn_blocking(move || backup::read_runelite(&from).and_then(|c| backup::put_runelite(&to, &c)))
        .await
        .map_err(|e| e.to_string())?
}

/// Forget where a character's window was, so the window manager places it next time.
#[tauri::command]
fn forget_window(st: State<'_, App>, pick: String) -> Result<(), String> {
    let mut store = st.store.lock().unwrap();
    if store.windows.remove(&pick).is_some() {
        st.save(&store)?;
    }
    Ok(())
}

fn emit_instances(app: &AppHandle) {
    let list = app.state::<App>().instances.lock().unwrap().clone();
    let _ = app.emit("instances", list);
    tray::refresh(app);
}

fn notify_enabled(st: &App) -> bool {
    st.store.lock().unwrap().settings.notifications
}

fn notify(summary: String, body: String) {
    tauri::async_runtime::spawn(async move { notify::send(&summary, &body).await });
}

/// Start a game. `jar`: use the custom jar (None: as the switch next to Play says). `remember`: make this the
/// character and client picked next time.
async fn start(
    app: &AppHandle,
    st: &App,
    client: Client,
    account_id: &str,
    character_id: &str,
    jar: Option<bool>,
    remember: bool,
) -> Result<Instance, String> {
    let (account, character) = {
        let store = st.store.lock().unwrap();
        let (a, c) = store
            .character(account_id, character_id)
            .ok_or("Pick a character first.")?;
        (a.clone(), c.clone())
    };
    let session = check_account(st, account_id, Duration::from_secs(4))
        .await?
        .ok_or_else(|| format!("The session for {} has expired. Sign in again to play.", account.label))?;
    {
        let _guard = st.downloads.lock().await;
        clients::ensure(&st.http, client, false, &progress_emitter(app)).await?;
    }
    if client == Client::Rs3 {
        let missing = launch::missing_libs(&client.file());
        if !missing.is_empty() {
            let hint = if config::in_flatpak() {
                "This Flatpak build should include them; please report this."
            } else {
                "Install GTK 2 and OpenSSL 1.1 (Arch: gtk2 openssl-1.1; Debian/Ubuntu: libgtk2.0-0 libssl1.1), \
                 or use the Flatpak, which includes them."
            };
            return Err(format!("RuneScape 3 needs libraries this system doesn't have: {}. {hint}", missing.join(", ")));
        }
    }
    let mut settings = st.store.lock().unwrap().settings.clone();
    let opts = match client {
        Client::Runelite => Some(&mut settings.runelite),
        Client::Hdos => Some(&mut settings.hdos),
        Client::Rs3 => None,
    };
    let debug = opts.is_some_and(|o| {
        if let Some(j) = jar {
            o.use_jar = j;
        }
        o.debug
    });
    let custom = launch::custom_jar(client, &settings);
    // Debugging attaches to the JVM the jar runs in; the normal launchers start the game in another one.
    let debug_port = match (&custom, debug) {
        (Some(_), true) => {
            let taken: Vec<u16> = st.instances.lock().unwrap().iter().filter_map(|i| i.debug_port).collect();
            let base = settings.debug_port.max(1024);
            Some(launch::free_port(base, &taken).ok_or(format!("No free debugger port was found from {base} up."))?)
        }
        _ => None,
    };
    let pick = format!("{account_id}:{character_id}");
    let profile = if client == Client::Runelite {
        let info = st.store.lock().unwrap().characters.get(&pick).cloned();
        profiles::for_character(&settings, info.as_ref(), &character.display_name).filter(|p| profiles::valid_name(p))
    } else {
        None
    };
    if let Some(p) = &profile {
        let (dir, p) = (profiles::dir(&settings), p.clone());
        tauri::async_runtime::spawn_blocking(move || profiles::ensure(&dir, &p)).await.map_err(|e| e.to_string())??;
    }
    let who = launch::Who {
        session_id: &session,
        character_id: &character.account_id,
        display_name: &character.display_name,
        profile: profile.as_deref(),
    };
    let plan = launch::plan(client, &settings, &who, debug_port)?;
    let log = launch::log_path(client);
    let mut child = launch::spawn(&plan, &log)?;
    let inst = Instance {
        pid: child.id(),
        client,
        account_id: account.id.clone(),
        account_label: account.label.clone(),
        character_id: character_id.to_string(),
        character: if character.display_name.is_empty() { "New character".into() } else { character.display_name },
        started_at: now(),
        log: log.to_string_lossy().into(),
        custom_jar: custom.is_some(),
        jar_stamp: custom.as_deref().map_or(0, |j| launch::jar_stamp(j, &settings)),
        jar_path: custom.unwrap_or_default(),
        rebuilt: false,
        debug_port,
        ended_at: 0,
        exit_code: None,
        crashed: false,
        crash_line: String::new(),
    };
    st.instances.lock().unwrap().push(inst.clone());
    emit_instances(app);

    let (watch_app, pid) = (app.clone(), inst.pid);
    std::thread::spawn(move || {
        let status = child.wait().ok();
        while launch::group_alive(pid) {
            std::thread::sleep(Duration::from_secs(2));
        }
        game_ended(&watch_app, pid, status);
    });

    if remember {
        let mut store = st.store.lock().unwrap();
        let s = &mut store.settings;
        s.last_game = if client == Client::Rs3 { "rs3".into() } else { "osrs".into() };
        if client != Client::Rs3 {
            s.last_client = client.id().into();
        }
        s.last_character = format!("{}:{}", account.id, character_id);
        let minimize = s.minimize_on_launch;
        let _ = st.save(&store);
        if minimize {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.minimize();
            }
        }
    }
    Ok(inst)
}

/// A game's process group is gone: keep it with the ended ones, count its playtime, and tell the window (and the
/// desktop, if it crashed while Lodestone wasn't in front).
fn game_ended(app: &AppHandle, pid: u32, status: Option<std::process::ExitStatus>) {
    use std::os::unix::process::ExitStatusExt;
    let st = app.state::<App>();
    let stopped = st.stopping.lock().unwrap().remove(&pid);
    let removed = {
        let mut l = st.instances.lock().unwrap();
        l.iter().position(|i| i.pid == pid).map(|n| l.remove(n))
    };
    let Some(mut inst) = removed else { return };
    inst.ended_at = now();
    inst.exit_code = status.and_then(|s| s.code().or_else(|| s.signal().map(|n| 128 + n)));
    let failed = status.is_some_and(|s| !s.success());
    let quick = inst.ended_at.saturating_sub(inst.started_at) < 60;
    inst.crashed = !stopped && (failed || quick);
    if inst.crashed {
        let tail = launch::read_log(std::path::Path::new(&inst.log), u64::MAX, 64 * 1024).map(|(t, _)| t).unwrap_or_default();
        inst.crash_line = launch::last_error(&tail).unwrap_or_default();
    }
    playtime::record(&format!("{}:{}", inst.account_id, inst.character_id), inst.started_at, inst.ended_at);
    {
        let mut e = st.ended.lock().unwrap();
        e.push(inst.clone());
        let n = e.len();
        e.drain(..n.saturating_sub(20));
    }
    emit_instances(app);
    let _ = app.emit("instance-ended", &inst);
    let focused = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
    if inst.crashed && !focused && notify_enabled(&st) {
        let why = if inst.crash_line.is_empty() {
            match inst.exit_code {
                Some(c) if c != 0 => format!("It exited with code {c}."),
                _ => "It closed within a minute of starting.".into(),
            }
        } else {
            inst.crash_line.clone()
        };
        notify(format!("{} ({}) crashed", inst.character, inst.client.name()), why);
    }
}

#[tauri::command]
async fn launch(
    app: AppHandle,
    st: State<'_, App>,
    client: Client,
    account_id: String,
    character_id: String,
    jar: Option<bool>,
    remember: Option<bool>,
) -> Result<Instance, String> {
    start(&app, &st, client, &account_id, &character_id, jar, remember.unwrap_or(true)).await
}

#[tauri::command]
fn stop_instance(st: State<'_, App>, pid: u32) -> Result<(), String> {
    // Only ever signal process groups this app started.
    if !st.instances.lock().unwrap().iter().any(|i| i.pid == pid) {
        return Err("That game isn't running from here.".into());
    }
    st.stopping.lock().unwrap().insert(pid);
    launch::stop_group(pid);
    Ok(())
}

fn stop_everything(st: &App) {
    let pids: Vec<u32> = st.instances.lock().unwrap().iter().map(|i| i.pid).collect();
    for pid in pids {
        st.stopping.lock().unwrap().insert(pid);
        launch::stop_group(pid);
    }
}

#[tauri::command]
fn stop_all(st: State<'_, App>) {
    stop_everything(&st);
}

/// Stop a game (if it still runs) and start the same character again with the same client and jar.
#[tauri::command]
async fn restart_instance(app: AppHandle, st: State<'_, App>, pid: u32) -> Result<Instance, String> {
    let (inst, running) = st.find_instance(pid).ok_or("That game isn't known here any more.")?;
    if running {
        st.stopping.lock().unwrap().insert(pid);
        launch::stop_group(pid);
        let mut waited = 0;
        while st.instances.lock().unwrap().iter().any(|i| i.pid == pid) {
            match waited {
                60 => launch::kill_group(pid),
                100.. => return Err("The game didn't close; stop it and try again.".into()),
                _ => {}
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            waited += 1;
        }
    }
    start(&app, &st, inst.client, &inst.account_id, &inst.character_id, Some(inst.custom_jar), false).await
}

#[tauri::command]
fn playtime() -> playtime::Book {
    playtime::load(&playtime::path())
}

/// RuneLite's screenshots, newest first, for the rotating background.
#[tauri::command]
fn screenshot_list(st: State<'_, App>) -> Vec<String> {
    let settings = st.store.lock().unwrap().settings.clone();
    launch::screenshots(&settings, 200).into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
}

/// Save what the log viewer shows to a file in the logs folder; returns its path.
#[tauri::command]
fn save_log(text: String) -> Result<String, String> {
    let dir = config::data_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("saved-{}.log", now()));
    std::fs::write(&path, text).map_err(|e| format!("couldn't save the log: {e}"))?;
    Ok(path.to_string_lossy().into_owned())
}

#[derive(Clone, Serialize)]
struct BuildLine {
    line: String,
    err: bool,
}

/// Build the plugin projects from settings, streaming Gradle's output as "build-output" events, and copy their jars
/// into the sideloaded-plugins folder. Returns the jar names copied.
#[tauri::command]
async fn build_plugins(app: AppHandle, st: State<'_, App>) -> Result<Vec<String>, String> {
    {
        let mut b = st.build.lock().unwrap();
        if b.is_some() {
            return Err("A build is already running.".into());
        }
        *b = Some(0);
    }
    let settings = st.store.lock().unwrap().settings.clone();
    let worker = app.clone();
    let res = tauri::async_runtime::spawn_blocking(move || run_build(&worker, &settings)).await;
    *st.build.lock().unwrap() = None;
    res.map_err(|e| e.to_string())?
}

fn run_build(app: &AppHandle, settings: &Settings) -> Result<Vec<String>, String> {
    use std::io::BufRead;
    use std::os::unix::process::CommandExt;
    let st = app.state::<App>();
    let say = |line: String, err: bool| {
        let _ = app.emit("build-output", BuildLine { line, err });
    };
    let projects = gradle::projects(settings);
    if projects.is_empty() {
        return Err("Add your plugin project folders in Settings → Clients first.".into());
    }
    let task = gradle::task(settings);
    let mut jars = Vec::new();
    for p in &projects {
        let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if !p.is_dir() {
            return Err(format!("{} isn't a folder (or the Flatpak can't see it).", p.display()));
        }
        say(format!("> {name}: gradle {task}"), false);
        let mut cmd = gradle::command(p, &task);
        // SAFETY: as for games: its own process group, so Cancel stops Gradle and what it started.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let mut child = cmd.spawn().map_err(|e| format!("couldn't start Gradle for {name}: {e}"))?;
        {
            let mut b = st.build.lock().unwrap();
            if b.is_none() {
                launch::stop_group(child.id());
                return Err("Build cancelled.".into());
            }
            *b = Some(child.id());
        }
        let stderr = child.stderr.take();
        let errs = app.clone();
        let t = std::thread::spawn(move || {
            if let Some(e) = stderr {
                for line in std::io::BufReader::new(e).lines().map_while(Result::ok) {
                    let _ = errs.emit("build-output", BuildLine { line, err: true });
                }
            }
        });
        if let Some(out) = child.stdout.take() {
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                say(line, false);
            }
        }
        let _ = t.join();
        let status = child.wait().map_err(|e| e.to_string())?;
        {
            let mut b = st.build.lock().unwrap();
            if b.is_none() {
                return Err("Build cancelled.".into());
            }
            *b = Some(0);
        }
        if !status.success() {
            return Err(format!("The build of {name} failed."));
        }
        let found = gradle::built_jars(p);
        if found.is_empty() {
            return Err(format!("{name} built, but there's no plugin jar in {}.", p.join("build/libs").display()));
        }
        jars.extend(found);
    }
    let dir = launch::sideload_dir(settings);
    let names = gradle::install(&jars, &dir)?;
    say(format!("Copied {} to {}", names.join(", "), dir.display()), false);
    Ok(names)
}

#[tauri::command]
fn cancel_build(st: State<'_, App>) {
    if let Some(pid) = st.build.lock().unwrap().take() {
        if pid > 0 {
            launch::stop_group(pid);
        }
    }
}

/// A picture from disk as raw bytes (an ArrayBuffer on the JS side, no base64), for the background. Only pictures
/// are read.
#[tauri::command]
async fn background_image(path: String) -> Result<tauri::ipc::Response, String> {
    let path = launch::expand_home(path.trim());
    let bytes = tokio::fs::read(&path).await.map_err(|e| {
        let hint = if config::in_flatpak() { " (the Flatpak can read your Pictures and Documents folders)" } else { "" };
        format!("couldn't read {path}: {e}{hint}")
    })?;
    if bytes.len() > 25 << 20 {
        return Err("That picture is over 25 MB; please use a smaller one.".into());
    }
    launch::image_type(&bytes).ok_or("That file isn't a PNG, JPEG, WebP or GIF picture.")?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogChunk {
    text: String,
    /// Where the next read starts.
    offset: u64,
    path: String,
    /// The file doesn't exist (yet).
    missing: bool,
    running: bool,
}

/// Read new output from a game started here: its console output, or RuneLite's own client.log. Without `from`, the
/// end of the file.
#[tauri::command]
fn read_log(st: State<'_, App>, pid: u32, source: String, from: Option<u64>) -> Result<LogChunk, String> {
    let (inst, running) = st.find_instance(pid).ok_or("That game's log isn't available any more.")?;
    let path = match source.as_str() {
        "output" => std::path::PathBuf::from(&inst.log),
        "client" if inst.client == Client::Runelite => launch::client_log(&st.store.lock().unwrap().settings),
        _ => return Err("unknown log".into()),
    };
    let shown = path.to_string_lossy().into_owned();
    match launch::read_log(&path, from.unwrap_or(u64::MAX), 512 * 1024) {
        Ok((text, offset)) => Ok(LogChunk { text, offset, path: shown, missing: false, running }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(LogChunk { text: String::new(), offset: from.unwrap_or(0), path: shown, missing: true, running })
        }
        Err(e) => Err(format!("couldn't read {shown}: {e}")),
    }
}

#[tauri::command]
fn open_folder(st: State<'_, App>, which: String) -> Result<(), String> {
    let target: std::ffi::OsString = match which.as_str() {
        "website" => "https://github.com/M4S4T0-V01D/void-lodestone".into(),
        "releases" => update::RELEASES.into(),
        dir => {
            let dir = match dir {
                "logs" => config::data_dir().join("logs"),
                "data" => config::data_dir(),
                "clients" => config::data_dir().join("clients"),
                "plugins" => launch::sideload_dir(&st.store.lock().unwrap().settings),
                "screenshots" => launch::screenshot_dir(&st.store.lock().unwrap().settings),
                "backups" => backup::dir(),
                _ => return Err("unknown folder".into()),
            };
            let _ = std::fs::create_dir_all(&dir);
            dir.into()
        }
    };
    std::process::Command::new("xdg-open")
        .arg(&target)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("couldn't open the folder: {e}"))
}

/// Look for a newer Lodestone on GitHub now. Returns the latest release, newer or not.
#[tauri::command]
async fn check_update(app: AppHandle, st: State<'_, App>) -> Result<update::Release, String> {
    let rel = update::latest(&st.http).await?;
    *st.release.lock().unwrap() = Some(rel.clone());
    if rel.newer {
        let _ = app.emit("update-available", &rel);
    }
    Ok(rel)
}

/// The notes of the running version's release, for "What's new".
#[tauri::command]
async fn release_notes(st: State<'_, App>) -> Result<update::Release, String> {
    update::of_version(&st.http, env!("CARGO_PKG_VERSION")).await
}

/// Download and install the newer release, streaming "update-progress" events.
#[tauri::command]
async fn install_update(app: AppHandle, st: State<'_, App>) -> Result<(), String> {
    let rel = st.release.lock().unwrap().clone().filter(|r| r.newer).ok_or("There's no newer version to install.")?;
    if st.updating.swap(true, Ordering::SeqCst) {
        return Err("The update is already being installed.".into());
    }
    let emitter = app.clone();
    let emit = move |p: update::Progress| {
        let _ = emitter.emit("update-progress", p);
    };
    let res = update::install(&st.http, &rel, &emit).await;
    st.updating.store(false, Ordering::SeqCst);
    if let Err(e) = &res {
        emit(update::Progress { phase: "error", received: 0, total: 0, message: e.clone() });
    }
    res
}

/// Quit and start the newly installed version. Refused while games run, so they aren't taken down with it.
#[tauri::command]
fn restart_app(app: AppHandle, st: State<'_, App>) -> Result<(), String> {
    if !st.instances.lock().unwrap().is_empty() {
        return Err("Stop your games first, or restart Lodestone later; the update is already installed.".into());
    }
    update::relaunch()?;
    let _ = std::fs::remove_file(instance_socket());
    app.exit(0);
    Ok(())
}

/// Where a running Lodestone listens, so starting it again (say from the menu while it sits in the tray) shows that
/// one instead of opening a second.
fn instance_socket() -> std::path::PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from);
    match (runtime, std::env::var_os("FLATPAK_ID")) {
        (Some(r), Some(id)) => r.join("app").join(id).join("lodestone.sock"),
        (Some(r), None) => r.join("void-lodestone.sock"),
        (None, _) => config::data_dir().join("lodestone.sock"),
    }
}

/// True if another Lodestone was found (and asked to show itself).
fn wake_other() -> bool {
    use std::io::Write;
    match std::os::unix::net::UnixStream::connect(instance_socket()) {
        Ok(mut s) => s.write_all(b"show\n").is_ok(),
        Err(_) => false,
    }
}

fn listen_for_others(app: AppHandle) {
    let path = instance_socket();
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(listener) = std::os::unix::net::UnixListener::bind(&path) else { return };
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            if conn.is_ok() {
                tray::show_window(&app);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if wake_other() {
        return;
    }
    // WebKitGTK's DMA-BUF renderer crashes on some Wayland + NVIDIA setups ("Error 71 (Protocol
    // error)"). Fall back to the shared-memory renderer unless the user chose otherwise.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    let http = reqwest::Client::builder()
        .user_agent(concat!("VOID-Lodestone/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("HTTP client");
    let (win_tx, win_rx) = std::sync::mpsc::channel();
    let state = App {
        store: Mutex::new(Store::load_from(&Store::path())),
        secrets: tokio::sync::OnceCell::new(),
        http,
        login_open: Mutex::new(false),
        expired: Mutex::new(HashSet::new()),
        health: Mutex::new(HashMap::new()),
        has_session: Mutex::new(HashMap::new()),
        instances: Arc::new(Mutex::new(Vec::new())),
        ended: Arc::new(Mutex::new(Vec::new())),
        downloads: tokio::sync::Mutex::new(()),
        stopping: Mutex::new(HashSet::new()),
        build: Mutex::new(None),
        tray: AtomicBool::new(false),
        release: Mutex::new(None),
        updating: AtomicBool::new(false),
        windows: Mutex::new(win_tx),
    };
    tauri::Builder::default()
        .manage(state)
        .on_window_event(|window, event| {
            // While games run, closing the window keeps Lodestone in the tray so they stay listed and stoppable.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let st = window.state::<App>();
                if window.label() == "main"
                    && st.tray.load(Ordering::Relaxed)
                    && st.store.lock().unwrap().settings.close_to_tray
                    && !st.instances.lock().unwrap().is_empty()
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            listen_for_others(app.handle().clone());
            let handle = app.handle().clone();
            std::thread::spawn(move || window_thread(handle, win_rx));
            if tray::available() {
                match tray::setup(app.handle()) {
                    Ok(()) => app.state::<App>().tray.store(true, Ordering::Relaxed),
                    Err(e) => eprintln!("no tray icon: {e}"),
                }
            }
            // Quietly keep installed clients current; nothing new is downloaded until it's first used. Each client
            // is checked again once its last check is stale.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut warned = HashSet::new();
                loop {
                    let st = handle.state::<App>();
                    {
                        let _guard = st.downloads.lock().await;
                        let emit = progress_emitter(&handle);
                        for c in Client::ALL {
                            if clients::status(c).installed {
                                match clients::ensure(&st.http, c, false, &emit).await {
                                    // Say so once, not every hour.
                                    Err(e) if warned.insert(c) && notify_enabled(&st) => {
                                        notify(format!("{} update check failed", c.name()), e)
                                    }
                                    Err(_) => {}
                                    Ok(_) => {
                                        warned.remove(&c);
                                    }
                                }
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                }
            });
            // Look for a new Lodestone shortly after start-up and every six hours, telling the user once per version.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut told = String::new();
                tokio::time::sleep(Duration::from_secs(5)).await;
                loop {
                    let st = handle.state::<App>();
                    if st.store.lock().unwrap().settings.check_updates {
                        if let Ok(rel) = update::latest(&st.http).await {
                            *st.release.lock().unwrap() = Some(rel.clone());
                            if rel.newer && told != rel.version {
                                told = rel.version.clone();
                                let _ = handle.emit("update-available", &rel);
                                if notify_enabled(&st) {
                                    notify(format!("VOID Lodestone {} is out", rel.version), "Open Lodestone to install it.".into());
                                }
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
                }
            });
            // Re-check sessions now and then, so an expired one shows up before Play is pressed. The window does
            // its own check at start-up.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(15 * 60)).await;
                    let st = handle.state::<App>();
                    if let Ok(lost) = check_all(&st).await {
                        if !lost.is_empty() && notify_enabled(&st) {
                            let body = format!("Sign in again to play: {}.", lost.join(", "));
                            notify("Jagex session expired".into(), body);
                        }
                        let _ = handle.emit("sessions-checked", lost);
                    }
                }
            });
            // Notice when the custom jar or a sideloaded plugin is rebuilt under a running game.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    let st = handle.state::<App>();
                    let watched: Vec<(u32, String, u64)> = st
                        .instances
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|i| i.custom_jar && !i.rebuilt)
                        .map(|i| (i.pid, i.jar_path.clone(), i.jar_stamp))
                        .collect();
                    if watched.is_empty() {
                        continue;
                    }
                    let settings = st.store.lock().unwrap().settings.clone();
                    let changed: Vec<u32> =
                        watched.into_iter().filter(|(_, jar, at)| launch::jar_stamp(jar, &settings) > *at).map(|w| w.0).collect();
                    if !changed.is_empty() {
                        for i in st.instances.lock().unwrap().iter_mut().filter(|i| changed.contains(&i.pid)) {
                            i.rebuilt = true;
                        }
                        emit_instances(&handle);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_settings,
            save_character,
            runelite_profiles,
            share_profile,
            tile_windows,
            restore_windows,
            monitors,
            forget_window,
            news,
            server_status,
            open_news,
            backups,
            make_backup,
            restore_backup,
            bolt_folders,
            import_bolt,
            add_account,
            reauth_account,
            submit_login_url,
            cancel_login,
            remove_account,
            rename_account,
            move_account,
            refresh_accounts,
            update_clients,
            rollback_client,
            resume_updates,
            launch,
            stop_instance,
            stop_all,
            restart_instance,
            playtime,
            screenshot_list,
            save_log,
            build_plugins,
            cancel_build,
            read_log,
            background_image,
            open_folder,
            check_update,
            release_notes,
            install_update,
            restart_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VOID Lodestone");
}
