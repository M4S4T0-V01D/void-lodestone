//! Desktop notifications through the freedesktop D-Bus service (KDE's, GNOME's or any other). Best effort: when
//! there's no service, nothing happens.

use std::collections::HashMap;
use zbus::zvariant::Value;

fn app_id() -> String {
    std::env::var("FLATPAK_ID").unwrap_or_else(|_| "void-lodestone".into())
}

pub async fn send(summary: &str, body: &str) {
    let Ok(conn) = zbus::Connection::session().await else { return };
    let id = app_id();
    let mut hints: HashMap<&str, Value> = HashMap::new();
    hints.insert("desktop-entry", Value::from(id.as_str()));
    let _ = conn
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &("VOID Lodestone", 0u32, id.as_str(), summary, body, Vec::<&str>::new(), hints, -1i32),
        )
        .await;
}
