//! Where things live and what's saved: settings and the account list (labels and characters, never secrets) in
//! $XDG_CONFIG_HOME/void-lodestone/config.json; clients, logs and sign-in cookies under $XDG_DATA_HOME/void-lodestone.
//! Inside the Flatpak those XDG folders are the app's own ~/.var/app folders.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::jagex::Character;

fn xdg(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var) {
        Some(v) if Path::new(&v).is_absolute() => PathBuf::from(v),
        _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into())).join(fallback),
    }
}

pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("void-lodestone")
}

pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("void-lodestone")
}

pub fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientOpts {
    /// Extra JVM arguments (RuneLite, HDOS) or nothing (RS3).
    pub jvm_args: String,
    /// Extra arguments for the client itself.
    pub args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String,
    /// Empty: the bundled JRE in the Flatpak, else $JAVA_HOME, else `java` on PATH.
    pub java_path: String,
    /// Empty: the real home outside the Flatpak (so existing ~/.runelite etc. are used), the app's data folder inside.
    pub game_home: String,
    /// A launch wrapper, e.g. `gamemoderun` or `mangohud --dlsym %command%`.
    pub wrapper: String,
    pub runelite: ClientOpts,
    pub hdos: ClientOpts,
    pub rs3: ClientOpts,
    pub rs3_config_uri: String,
    pub minimize_on_launch: bool,
    pub reduce_motion: bool,
    pub last_game: String,
    pub last_client: String,
    pub last_character: String,
}

pub const RS3_CONFIG_URI: &str = "https://rs.config.runescape.com/k=5/l=$(Language:0)/jav_config.ws";

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "void".into(),
            java_path: String::new(),
            game_home: String::new(),
            wrapper: String::new(),
            runelite: ClientOpts::default(),
            hdos: ClientOpts::default(),
            rs3: ClientOpts::default(),
            rs3_config_uri: RS3_CONFIG_URI.into(),
            minimize_on_launch: false,
            reduce_motion: false,
            last_game: "osrs".into(),
            last_client: "runelite".into(),
            last_character: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Local id; also names the keyring entry and the sign-in cookie folder.
    pub id: String,
    pub label: String,
    /// The Jagex `sub` claim, so signing in to the same account twice updates it instead of adding a copy.
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub characters: Vec<Character>,
    #[serde(default)]
    pub added_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    pub settings: Settings,
    pub accounts: Vec<Account>,
}

impl Store {
    pub fn path() -> PathBuf {
        config_dir().join("config.json")
    }

    /// A missing or unreadable file gives the defaults; a corrupt one is kept aside rather than overwritten.
    pub fn load_from(path: &Path) -> Store {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                let _ = std::fs::rename(path, path.with_extension("json.bad"));
                Store::default()
            }),
            Err(_) => Store::default(),
        }
    }

    /// Atomic write (temp file + rename), readable by the user only.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        {
            let mut f = private_file(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(self).expect("settings serialise"))?;
            f.sync_all()?;
        }
        std::fs::rename(tmp, path)
    }

    pub fn character(&self, account_id: &str, character_id: &str) -> Option<(&Account, &Character)> {
        let a = self.accounts.iter().find(|a| a.id == account_id)?;
        let c = a.characters.iter().find(|c| c.account_id == character_id)?;
        Some((a, c))
    }
}

pub fn private_file(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lodestone-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("config.json")
    }

    #[test]
    fn round_trips_and_is_private() {
        let p = tmp("rt");
        let mut s = Store::default();
        s.accounts.push(Account {
            id: "a".into(),
            label: "Main".into(),
            subject: "sub".into(),
            characters: vec![Character { account_id: "c1".into(), display_name: "Zezima".into() }],
            added_at: 1,
        });
        s.save_to(&p).unwrap();
        assert_eq!(Store::load_from(&p), s);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(s.character("a", "c1").unwrap().1.display_name, "Zezima");
    }

    #[test]
    fn corrupt_file_is_kept_aside() {
        let p = tmp("bad");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"{not json").unwrap();
        assert_eq!(Store::load_from(&p), Store::default());
        assert!(p.with_extension("json.bad").exists());
    }

    #[test]
    fn old_files_gain_new_defaults() {
        let p = tmp("partial");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, br#"{"settings":{"theme":"abyss"}}"#).unwrap();
        let s = Store::load_from(&p);
        assert_eq!(s.settings.theme, "abyss");
        assert_eq!(s.settings.rs3_config_uri, RS3_CONFIG_URI);
    }
}
