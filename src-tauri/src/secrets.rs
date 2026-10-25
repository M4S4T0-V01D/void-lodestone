//! Game-session ids are the only secrets kept, one per account. They go in the desktop keyring (Secret Service on
//! the host, the Secret portal inside the Flatpak). Without a keyring they fall back to a file only the user can
//! read, and the UI says so.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::config::{data_dir, private_file};

const APP: &str = "void-lodestone";

pub enum Backend {
    Keyring(oo7::Keyring),
    File(PathBuf),
}

impl Backend {
    pub async fn open() -> Backend {
        match tokio::time::timeout(std::time::Duration::from_secs(5), oo7::Keyring::new()).await {
            Ok(Ok(k)) => Backend::Keyring(k),
            _ => Backend::File(data_dir().join("sessions.json")),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Backend::Keyring(_) => "keyring",
            Backend::File(_) => "file",
        }
    }

    fn attrs(account: &str) -> Vec<(&str, &str)> {
        vec![("application", APP), ("account", account)]
    }

    pub async fn get(&self, account: &str) -> Option<String> {
        match self {
            Backend::Keyring(k) => {
                let _ = k.unlock().await;
                let items = k.search_items(&Self::attrs(account)).await.ok()?;
                let item = items.first()?;
                let secret = item.secret().await.ok()?;
                String::from_utf8(secret.as_bytes().to_vec()).ok()
            }
            Backend::File(p) => read_file(p).remove(account),
        }
    }

    pub async fn set(&self, account: &str, session: &str) -> Result<(), String> {
        match self {
            Backend::Keyring(k) => {
                let _ = k.unlock().await;
                k.create_item(
                    &format!("VOID Lodestone: Jagex session ({account})"),
                    &Self::attrs(account),
                    session,
                    true,
                )
                .await
                .map_err(|e| format!("couldn't save to the keyring: {e}"))
            }
            Backend::File(p) => {
                let mut m = read_file(p);
                m.insert(account.to_string(), session.to_string());
                write_file(p, &m)
            }
        }
    }

    pub async fn delete(&self, account: &str) {
        match self {
            Backend::Keyring(k) => {
                let _ = k.delete(&Self::attrs(account)).await;
            }
            Backend::File(p) => {
                let mut m = read_file(p);
                if m.remove(account).is_some() {
                    let _ = write_file(p, &m);
                }
            }
        }
    }
}

fn read_file(p: &PathBuf) -> BTreeMap<String, String> {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn write_file(p: &PathBuf, m: &BTreeMap<String, String>) -> Result<(), String> {
    use std::io::Write;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("json.tmp");
    let mut f = private_file(&tmp).map_err(|e| e.to_string())?;
    f.write_all(&serde_json::to_vec(m).unwrap()).map_err(|e| e.to_string())?;
    drop(f);
    std::fs::rename(tmp, p).map_err(|e| e.to_string())
}
