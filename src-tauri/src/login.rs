//! The sign-in window. Jagex's own pages load in a separate webview that has no access to the app (no IPC
//! capability), with its own cookie folder per account so "sign in again" can often pass straight through. The two
//! redirects that carry secrets are caught before they're requested and cancelled, so codes and tokens never leave
//! the process and nothing listens on a local port.
//!
//! If that window doesn't work on a system, the same flow runs in the user's own browser instead: the pages open
//! there, and the user pastes the address each leg ends on back into the app, which reads the code or token from it.

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::mpsc;

use crate::config::data_dir;
use crate::jagex::{self, Character, Redirect};

pub const WINDOW: &str = "login";

pub fn cookie_dir(account_id: &str) -> PathBuf {
    data_dir().join("webview").join(account_id)
}

pub struct Outcome {
    pub subject: String,
    pub session_id: String,
    pub characters: Vec<Character>,
}

#[derive(Clone, serde::Serialize)]
pub struct Status {
    /// waiting | verifying | done | error | cancelled
    pub phase: &'static str,
    pub message: String,
}

fn status(app: &AppHandle, phase: &'static str, message: impl Into<String>) {
    let _ = app.emit("login-status", Status { phase, message: message.into() });
}

#[derive(Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Window,
    Browser,
}

enum Msg {
    Nav(Redirect),
    Closed,
}

/// The running sign-in's channel and states, so pasted addresses and Cancel can reach it.
struct Pending {
    tx: mpsc::UnboundedSender<Msg>,
    launcher_state: String,
    consent_state: String,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn close(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(WINDOW) {
        let _ = w.destroy();
    }
}

fn open_in_browser(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("couldn't open your browser: {e}"))
}

/// Run a full sign-in for `account_id` (new or existing). Only one runs at a time; the caller enforces that.
pub async fn run(app: AppHandle, http: reqwest::Client, account_id: String, mode: Mode) -> Result<Outcome, String> {
    let res = flow(&app, &http, &account_id, mode).await;
    *PENDING.lock().unwrap() = None;
    close(&app);
    match &res {
        Ok(_) => status(&app, "done", ""),
        Err(e) if e == "cancelled" => status(&app, "cancelled", ""),
        Err(e) => status(&app, "error", e.clone()),
    }
    res
}

async fn flow(app: &AppHandle, http: &reqwest::Client, account_id: &str, mode: Mode) -> Result<Outcome, String> {
    let pkce = jagex::Pkce::new();
    let launcher_state = jagex::random_token(24);
    let consent_state = jagex::random_token(24);
    let nonce = jagex::random_token(24);
    let start = jagex::launcher_auth_url(&launcher_state, &pkce.challenge);

    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    *PENDING.lock().unwrap() = Some(Pending {
        tx: tx.clone(),
        launcher_state: launcher_state.clone(),
        consent_state: consent_state.clone(),
    });
    close(app);
    let window = match mode {
        Mode::Browser => {
            open_in_browser(&start)?;
            status(
                app,
                "waiting",
                "Sign in with your Jagex account in your browser. When it finishes (the page may ask to open a \
                 \"jagex\" link: you can cancel that), copy the address from the address bar and paste it here.",
            );
            None
        }
        Mode::Window => {
            let nav_tx = tx.clone();
            let (ls, cs) = (launcher_state.clone(), consent_state.clone());
            let window = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::External(start.parse().unwrap()))
                .title("Sign in to your Jagex account · VOID Lodestone")
                .inner_size(520.0, 780.0)
                .min_inner_size(400.0, 560.0)
                .data_directory(cookie_dir(account_id))
                .on_navigation(move |url| match jagex::classify(url.as_str(), &ls, &cs) {
                    Redirect::Pass => true,
                    other => {
                        let _ = nav_tx.send(Msg::Nav(other));
                        false
                    }
                })
                .build()
                .map_err(|e| format!("couldn't open the sign-in window: {e}"))?;
            window.on_window_event(move |ev| {
                if matches!(ev, tauri::WindowEvent::Destroyed) {
                    let _ = tx.send(Msg::Closed);
                }
            });
            status(app, "waiting", "Sign in with your Jagex account in the window that opened.");
            Some(window)
        }
    };

    let mut subject = String::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20 * 60);
    loop {
        let msg = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .map_err(|_| "the sign-in timed out".to_string())?
            .unwrap_or(Msg::Closed);
        match msg {
            Msg::Closed => return Err("cancelled".into()),
            Msg::Nav(Redirect::Pass) => {}
            Msg::Nav(Redirect::Failed(why)) => return Err(format!("Jagex sign-in failed: {why}")),
            Msg::Nav(Redirect::Code(code)) => {
                if !subject.is_empty() {
                    continue; // a repeated redirect for a code already used
                }
                status(app, "verifying", "Checking your sign-in…");
                let tokens = jagex::exchange_code(http, &code, &pkce.verifier).await.map_err(|e| e.to_string())?;
                let claims = jagex::jwt_claims(&tokens.id_token).unwrap_or_default();
                if claims["login_provider"] == "runescape" {
                    return Err("This is a legacy RuneScape account, not a Jagex account. VOID Lodestone signs in \
                                with Jagex accounts only; you can migrate the account on runescape.com."
                        .into());
                }
                subject = claims["sub"].as_str().unwrap_or("").to_string();
                if subject.is_empty() {
                    subject = "unknown".into();
                }
                let next = jagex::consent_url(&consent_state, &nonce);
                match &window {
                    Some(w) => {
                        w.navigate(next.parse().unwrap()).map_err(|e| e.to_string())?;
                        status(app, "waiting", "Confirm access to your game characters.");
                    }
                    None => {
                        open_in_browser(&next)?;
                        status(
                            app,
                            "waiting",
                            "Step 2 of 2: allow access to your game characters in the browser tab that just opened. \
                             It then ends on a \"localhost\" page that can't load; copy that page's address and paste \
                             it here.",
                        );
                    }
                }
            }
            Msg::Nav(Redirect::IdToken(id_token)) => {
                if subject.is_empty() {
                    return Err("the sign-in replies arrived out of order; please try again".into());
                }
                let claims = jagex::jwt_claims(&id_token).unwrap_or_default();
                if claims["nonce"].as_str() != Some(nonce.as_str()) {
                    return Err("the sign-in reply didn't match this request (nonce mismatch)".into());
                }
                status(app, "verifying", "Opening a game session…");
                let session_id = jagex::create_session(http, &id_token).await.map_err(|e| e.to_string())?;
                let characters = jagex::characters(http, &session_id).await.map_err(|e| e.to_string())?;
                return Ok(Outcome { subject, session_id, characters });
            }
        }
    }
}

/// An address pasted from the browser during a browser sign-in.
pub fn submit(url: &str) -> Result<(), String> {
    let pending = PENDING.lock().unwrap();
    let p = pending.as_ref().ok_or("No sign-in is waiting for an address.")?;
    match jagex::classify(url.trim(), &p.launcher_state, &p.consent_state) {
        Redirect::Pass => Err("That isn't the address the sign-in ended on. Copy the whole address from the \
                               browser's address bar once the page has finished."
            .into()),
        r => p.tx.send(Msg::Nav(r)).map_err(|_| "The sign-in already ended.".into()),
    }
}

pub fn cancel(app: &AppHandle) {
    if let Some(p) = PENDING.lock().unwrap().as_ref() {
        let _ = p.tx.send(Msg::Closed);
    }
    close(app);
}
