//! VOID Lodestone: a launcher for RuneLite, HDOS and RuneScape 3 with any number of Jagex accounts.
//! No telemetry: the only hosts contacted are Jagex's (sign-in, sessions, RS3 download), GitHub (RuneLite) and
//! HDOS's CDN.

mod clients;
mod config;
mod jagex;
mod launch;
mod login;
mod secrets;

use serde::Serialize;
use std::collections::HashSet;
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
    instances: Arc<Mutex<Vec<Instance>>>,
    /// One download at a time, so a launch and an update never write the same file.
    downloads: tokio::sync::Mutex<()>,
}

impl App {
    async fn secrets(&self) -> &secrets::Backend {
        self.secrets.get_or_init(secrets::Backend::open).await
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pid: u32,
    client: Client,
    account_id: String,
    account_label: String,
    character: String,
    started_at: u64,
    log: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountView {
    id: String,
    label: String,
    characters: Vec<Character>,
    added_at: u64,
    signed_in: bool,
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
}

async fn view(st: &App) -> StateView {
    let store = st.store.lock().unwrap().clone();
    let backend = st.secrets().await;
    let mut accounts = Vec::new();
    for a in store.accounts {
        let expired = st.expired.lock().unwrap().contains(&a.id);
        let signed_in = !expired && backend.get(&a.id).await.is_some();
        accounts.push(AccountView {
            id: a.id,
            label: a.label,
            characters: a.characters,
            added_at: a.added_at,
            signed_in,
        });
    }
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
    }
}

#[tauri::command]
async fn get_state(st: State<'_, App>) -> Result<StateView, String> {
    Ok(view(&st).await)
}

#[tauri::command]
fn save_settings(st: State<'_, App>, settings: Settings) -> Result<(), String> {
    let mut store = st.store.lock().unwrap();
    store.settings = settings;
    st.save(&store)
}

async fn sign_in(app: AppHandle, st: &App, existing: Option<String>) -> Result<StateView, String> {
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
    let res = login::run(app.clone(), st.http.clone(), id.clone()).await;
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
                    });
                    id.clone()
                }
            },
        };
        let a = store.accounts.iter_mut().find(|a| a.id == target).unwrap();
        a.subject = out.subject.clone();
        a.characters = out.characters.clone();
        st.save(&store)?;
        target
    };
    st.secrets().await.set(&target, &out.session_id).await?;
    st.set_expired(&target, false);
    Ok(view(st).await)
}

#[tauri::command]
async fn add_account(app: AppHandle, st: State<'_, App>) -> Result<StateView, String> {
    sign_in(app, &st, None).await
}

#[tauri::command]
async fn reauth_account(app: AppHandle, st: State<'_, App>, id: String) -> Result<StateView, String> {
    sign_in(app, &st, Some(id)).await
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
    st.set_expired(&id, false);
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
    let Some(session) = st.secrets().await.get(id).await else {
        st.set_expired(id, true);
        return Ok(None);
    };
    match tokio::time::timeout(wait, jagex::characters(&st.http, &session)).await {
        Ok(Ok(chars)) => {
            st.set_expired(id, false);
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
            Ok(None)
        }
        // Offline or Jagex is slow: keep what we have and let the game client try.
        _ => Ok(Some(session)),
    }
}

#[tauri::command]
async fn refresh_accounts(st: State<'_, App>) -> Result<StateView, String> {
    let ids: Vec<String> = st.store.lock().unwrap().accounts.iter().map(|a| a.id.clone()).collect();
    for id in ids {
        check_account(&st, &id, Duration::from_secs(10)).await?;
    }
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

#[tauri::command]
async fn launch(
    app: AppHandle,
    st: State<'_, App>,
    client: Client,
    account_id: String,
    character_id: String,
) -> Result<Instance, String> {
    let (account, character) = {
        let store = st.store.lock().unwrap();
        let (a, c) = store
            .character(&account_id, &character_id)
            .ok_or("Pick a character first.")?;
        (a.clone(), c.clone())
    };
    let session = check_account(&st, &account_id, Duration::from_secs(4))
        .await?
        .ok_or_else(|| format!("The session for {} has expired. Sign in again to play.", account.label))?;
    {
        let _guard = st.downloads.lock().await;
        clients::ensure(&st.http, client, false, &progress_emitter(&app)).await?;
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
    let settings = st.store.lock().unwrap().settings.clone();
    let who = launch::Who {
        session_id: &session,
        character_id: &character.account_id,
        display_name: &character.display_name,
    };
    let plan = launch::plan(client, &settings, &who)?;
    let log = launch::log_path(client);
    let mut child = launch::spawn(&plan, &log)?;
    let inst = Instance {
        pid: child.id(),
        client,
        account_id: account.id.clone(),
        account_label: account.label.clone(),
        character: if character.display_name.is_empty() { "New character".into() } else { character.display_name },
        started_at: now(),
        log: log.to_string_lossy().into(),
    };
    st.instances.lock().unwrap().push(inst.clone());
    let _ = app.emit("instances", st.instances.lock().unwrap().clone());

    let (watch_app, list, pid) = (app.clone(), st.instances.clone(), inst.pid);
    std::thread::spawn(move || {
        let _ = child.wait();
        while launch::group_alive(pid) {
            std::thread::sleep(Duration::from_secs(2));
        }
        let mut l = list.lock().unwrap();
        l.retain(|i| i.pid != pid);
        let _ = watch_app.emit("instances", l.clone());
    });

    {
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

#[tauri::command]
fn stop_instance(st: State<'_, App>, pid: u32) -> Result<(), String> {
    // Only ever signal process groups this app started.
    if !st.instances.lock().unwrap().iter().any(|i| i.pid == pid) {
        return Err("That game isn't running from here.".into());
    }
    launch::stop_group(pid);
    Ok(())
}

#[tauri::command]
fn open_folder(which: String) -> Result<(), String> {
    let target: std::ffi::OsString = match which.as_str() {
        "website" => "https://github.com/M4S4T0-V01D/void-lodestone".into(),
        dir => {
            let dir = match dir {
                "logs" => config::data_dir().join("logs"),
                "data" => config::data_dir(),
                "clients" => config::data_dir().join("clients"),
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
    let state = App {
        store: Mutex::new(Store::load_from(&Store::path())),
        secrets: tokio::sync::OnceCell::new(),
        http,
        login_open: Mutex::new(false),
        expired: Mutex::new(HashSet::new()),
        instances: Arc::new(Mutex::new(Vec::new())),
        downloads: tokio::sync::Mutex::new(()),
    };
    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            // Quietly keep installed clients current; nothing new is downloaded until it's first used.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let st = handle.state::<App>();
                let _guard = st.downloads.lock().await;
                let emit = progress_emitter(&handle);
                for c in Client::ALL {
                    if clients::status(c).installed {
                        let _ = clients::ensure(&st.http, c, false, &emit).await;
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_settings,
            add_account,
            reauth_account,
            cancel_login,
            remove_account,
            rename_account,
            move_account,
            refresh_accounts,
            update_clients,
            launch,
            stop_instance,
            open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VOID Lodestone");
}
