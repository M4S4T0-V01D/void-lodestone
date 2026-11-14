//! Where things live and what's saved: settings and the account list (labels and characters, never secrets) in
//! $XDG_CONFIG_HOME/void-lodestone/config.json; clients, logs and sign-in cookies under $XDG_DATA_HOME/void-lodestone.
//! Inside the Flatpak those XDG folders are the app's own ~/.var/app folders.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// A jar to run instead of the downloaded one (RuneLite, HDOS), such as a developer-mode bootstrap. Empty: the
    /// downloaded client.
    pub jar_path: String,
    /// Whether launches use `jar_path` (the Custom jar / Normal switch next to Play).
    pub use_jar: bool,
    /// Start custom-jar launches with a Java debugger port open (JDWP on localhost).
    pub debug: bool,
}

/// One character in a saved group: `pick` is `account id:character id`; `client` and `jar` are for Old School.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GroupRow {
    pub pick: String,
    pub client: String,
    pub jar: bool,
}

/// Characters started together with one click.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Group {
    pub name: String,
    /// "osrs" or "rs3".
    pub game: String,
    pub rows: Vec<GroupRow>,
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
    /// A picture behind the middle of the window; empty for none. Only ever read from the user's own disk.
    pub background: String,
    /// How much the background picture is darkened, in percent, so text stays readable.
    pub background_dim: u8,
    /// The RuneScape 3 tab's picture; empty uses `background`.
    pub background_rs3: String,
    /// Show a different RuneLite screenshot every few minutes instead of a fixed picture.
    pub background_rotate: bool,
    /// Background blur in pixels.
    pub background_blur: u8,
    /// Which part of the picture stays in view: "center", "top" or "bottom".
    pub background_position: String,
    /// Minutes between rotating screenshots.
    pub rotate_minutes: u16,
    /// A colour (#rrggbb) used instead of the theme's accent; empty for the theme's own.
    pub accent: String,
    /// Page zoom in percent.
    pub ui_scale: u16,
    /// "comfortable" or "compact".
    pub density: String,
    /// Corner roundness in pixels.
    pub radius: u8,
    /// The big logo and title above the clients.
    pub show_hero: bool,
    /// Side panels folded away from the top bar.
    pub hide_accounts: bool,
    pub hide_instances: bool,
    /// The first JDWP port for debug launches; more games at once take the next free ones.
    pub debug_port: u16,
    /// RuneLite plugin projects (Gradle), one folder per line, built by "Build & play".
    pub plugin_projects: String,
    /// The Gradle task that makes each plugin's jar.
    pub build_task: String,
    /// Desktop notifications for crashes, expired sessions and failed update checks.
    pub notifications: bool,
    /// Closing the window hides it to the tray (when there is one) instead of quitting.
    pub close_to_tray: bool,
    /// Look for new Lodestone releases on GitHub at start-up and every few hours.
    pub check_updates: bool,
    /// The version whose release notes were last shown, so "What's new" appears once after an update.
    pub seen_version: String,
    pub groups: Vec<Group>,
    /// Start each character's RuneLite with its own settings profile, made from the active one the first time.
    pub runelite_profiles: bool,
    /// Put the character's name in the game window's title.
    pub window_titles: bool,
    /// Put each character's game window back where it was last time.
    pub remember_windows: bool,
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
            background: String::new(),
            background_dim: 55,
            background_rs3: String::new(),
            background_rotate: false,
            background_blur: 0,
            background_position: "center".into(),
            rotate_minutes: 5,
            accent: String::new(),
            ui_scale: 100,
            density: "comfortable".into(),
            radius: 10,
            show_hero: true,
            hide_accounts: false,
            hide_instances: false,
            debug_port: 5005,
            plugin_projects: String::new(),
            build_task: "jar".into(),
            notifications: true,
            close_to_tray: true,
            check_updates: true,
            seen_version: String::new(),
            groups: Vec::new(),
            runelite_profiles: false,
            window_titles: true,
            remember_windows: true,
            last_game: "osrs".into(),
            last_client: "runelite".into(),
            last_character: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Goal {
    pub text: String,
    pub done: bool,
}

/// What the user keeps about one character.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CharacterInfo {
    pub notes: String,
    pub goals: Vec<Goal>,
    pub tags: Vec<String>,
    /// #rrggbb, or empty for none.
    pub color: String,
    /// The RuneLite profile it plays with; empty: its own (when profiles per character are on) or RuneLite's active one.
    pub profile: String,
}

/// Where a character's game window was: the client area's top-left corner on the X screen and its size.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowSpot {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
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
    /// When the saved session was created (the last sign-in). 0 for accounts from older versions.
    #[serde(default)]
    pub signed_in_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    pub settings: Settings,
    pub accounts: Vec<Account>,
    /// Keyed by pick (`account id:character id`).
    pub characters: BTreeMap<String, CharacterInfo>,
    pub windows: BTreeMap<String, WindowSpot>,
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
            signed_in_at: 2,
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
        assert_eq!((s.settings.ui_scale, s.settings.radius, s.settings.rotate_minutes), (100, 10, 5));
        assert!(s.settings.show_hero && !s.settings.hide_accounts);
    }
}
