//! The tray icon: what's running, saved groups to start, Stop all. Linux trays go through libappindicator, which is
//! loaded at run time; without it there's simply no tray (and closing the window quits as usual).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use crate::App;

const ID: &str = "main";

/// Whether an appindicator library is there (tauri panics without one).
pub fn available() -> bool {
    ["libayatana-appindicator3.so.1", "libappindicator3.so.1"].iter().any(|lib| {
        let name = std::ffi::CString::new(*lib).unwrap();
        // SAFETY: dlopen with a valid C string; the handle is closed right away.
        unsafe {
            let h = libc::dlopen(name.as_ptr(), libc::RTLD_LAZY);
            if h.is_null() {
                false
            } else {
                libc::dlclose(h);
                true
            }
        }
    })
}

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let st = app.state::<App>();
    let running = st.instances.lock().unwrap().clone();
    let groups = st.store.lock().unwrap().settings.groups.clone();
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "show", "Show Lodestone", true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    if running.is_empty() {
        menu.append(&MenuItem::with_id(app, "none", "No games running", false, None::<&str>)?)?;
    }
    for i in &running {
        let text = format!("▶ {} · {}{}", i.character, i.client.name(), if i.debug_port.is_some() { " · debug" } else { "" });
        menu.append(&MenuItem::with_id(app, format!("run:{}", i.pid), text, false, None::<&str>)?)?;
    }
    if !groups.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        for (n, g) in groups.iter().enumerate() {
            menu.append(&MenuItem::with_id(app, format!("group:{n}"), format!("Start {}", g.name), true, None::<&str>)?)?;
        }
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "stop-all", "Stop all games", !running.is_empty(), None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit Lodestone", true, None::<&str>)?)?;
    Ok(menu)
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let mut b = TrayIconBuilder::with_id(ID)
        .tooltip("VOID Lodestone")
        .menu(&menu(app)?)
        .on_menu_event(|app, e| match e.id().as_ref() {
            "show" => show_window(app),
            "stop-all" => crate::stop_everything(&app.state::<App>()),
            "quit" => app.exit(0),
            id => {
                if let Some(n) = id.strip_prefix("group:").and_then(|n| n.parse::<usize>().ok()) {
                    // The window starts the group, the same way its own button does.
                    let _ = app.emit("tray-group", n);
                }
            }
        });
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(app)?;
    Ok(())
}

/// Rebuild the menu after games start or stop, or groups change.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(ID) else { return };
    if let Ok(m) = menu(app) {
        let _ = tray.set_menu(Some(m));
    }
    let n = app.state::<App>().instances.lock().unwrap().len();
    let _ = tray.set_tooltip(Some(match n {
        0 => "VOID Lodestone".to_string(),
        1 => "VOID Lodestone · 1 game running".to_string(),
        n => format!("VOID Lodestone · {n} games running"),
    }));
}
