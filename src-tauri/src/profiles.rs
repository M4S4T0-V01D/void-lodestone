//! RuneLite's settings profiles (<home>/.runelite/profiles2): one per character, made as a copy of the active one, and
//! copying one profile's settings over others. profiles.json is only changed under RuneLite's own lock (profiles.lck),
//! so profiles can be added while clients run. A profile's .properties is only written while no RuneLite runs: a
//! running client keeps its settings in memory and saves over the file.

use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::config::{CharacterInfo, Settings};
use crate::launch;

/// RuneLite's profile for account data (bank values, display names), not settings.
const RS_PROFILE: &str = "$rsprofile";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: i64,
    pub name: String,
    /// Kept in the RuneLite account's cloud storage, which wins over the file here.
    pub sync: bool,
    /// The one RuneLite opens when no profile is asked for.
    pub active: bool,
}

pub fn dir(settings: &Settings) -> PathBuf {
    launch::runelite_dir(settings).join("profiles2")
}

fn file(dir: &Path, name: &str, id: i64) -> PathBuf {
    dir.join(format!("{name}-{id}.properties"))
}

/// The profile a character plays with: the one picked for it, else its own when profiles per character are on.
pub fn for_character(settings: &Settings, info: Option<&CharacterInfo>, display_name: &str) -> Option<String> {
    match info.map(|i| i.profile.trim()).unwrap_or("") {
        "" if settings.runelite_profiles && !display_name.trim().is_empty() => Some(display_name.trim().to_string()),
        "" => None,
        p => Some(p.to_string()),
    }
}

/// A name RuneLite can keep in a file name.
pub fn valid_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty() && n.len() <= 64 && n != RS_PROFILE && !n.starts_with('.') && !n.contains(['/', '\\', '\0', '\n'])
}

/// RuneLite's lock: an exclusive fcntl lock on profiles.lck, which is what Java's FileChannel.lock takes on Linux.
struct Lock(#[allow(dead_code)] std::fs::File);

impl Lock {
    fn take(dir: &Path) -> Result<Lock, String> {
        use std::os::fd::AsRawFd;
        std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
        let f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("profiles.lck"))
            .map_err(|e| format!("couldn't lock RuneLite's profiles: {e}"))?;
        // SAFETY: a zeroed flock is valid (whole file from the start); fcntl only reads it.
        let mut fl: libc::flock = unsafe { std::mem::zeroed() };
        fl.l_type = libc::F_WRLCK as _;
        fl.l_whence = libc::SEEK_SET as _;
        for _ in 0..50 {
            if unsafe { libc::fcntl(f.as_raw_fd(), libc::F_SETLK, &fl) } == 0 {
                return Ok(Lock(f));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Err("RuneLite is busy with its profiles; try again in a moment.".into())
    }
}

/// profiles.json as it is, so fields this doesn't know about survive. None if RuneLite hasn't made it yet.
fn load(dir: &Path) -> Result<Option<Value>, String> {
    let bytes = match std::fs::read(dir.join("profiles.json")) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("couldn't read RuneLite's profiles: {e}")),
    };
    let root: Value = serde_json::from_slice(&bytes).map_err(|e| format!("RuneLite's profiles.json is damaged: {e}"))?;
    if !root.get("profiles").is_some_and(Value::is_array) {
        return Err("RuneLite's profiles.json has no profile list".into());
    }
    Ok(Some(root))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("lodestone-tmp");
    std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("couldn't write {}: {e}", path.display())
    })
}

fn entries(root: &Value) -> Vec<Profile> {
    root["profiles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            Some(Profile {
                id: p["id"].as_i64()?,
                name: p["name"].as_str()?.to_string(),
                sync: p["sync"].as_bool().unwrap_or(false),
                active: p["active"].as_bool().unwrap_or(false),
            })
        })
        .filter(|p| p.name != RS_PROFILE)
        .collect()
}

/// The settings profiles, without RuneLite's account-data one.
pub fn list(dir: &Path) -> Result<Vec<Profile>, String> {
    Ok(load(dir)?.map(|r| entries(&r)).unwrap_or_default())
}

/// Make the profile if it's missing, starting from a copy of the active one's settings. Returns whether it was made.
/// Before RuneLite's first start there is nothing to copy; RuneLite then makes the profile itself.
pub fn ensure(dir: &Path, name: &str) -> Result<bool, String> {
    if !valid_name(name) {
        return Err(format!("\"{name}\" can't be used as a RuneLite profile name."));
    }
    if !dir.join("profiles.json").exists() {
        return Ok(false);
    }
    let _lock = Lock::take(dir)?;
    let Some(mut root) = load(dir)? else { return Ok(false) };
    let have = entries(&root);
    if have.iter().any(|p| p.name == name) {
        return Ok(false);
    }
    let template = have.iter().find(|p| p.active).or_else(|| have.iter().find(|p| p.name == "default")).or(have.first());
    let mut id = (crate::config::now() as i64) << 20 | (std::process::id() as i64 & 0xfffff);
    while root["profiles"].as_array().unwrap().iter().any(|p| p["id"].as_i64() == Some(id)) {
        id += 1;
    }
    if let Some(t) = template {
        let from = file(dir, &t.name, t.id);
        if from.exists() {
            let bytes = std::fs::read(&from).map_err(|e| format!("couldn't read profile {}: {e}", t.name))?;
            write_atomic(&file(dir, name, id), &bytes)?;
        }
    }
    root["profiles"].as_array_mut().unwrap().push(json!({
        "id": id,
        "name": name,
        "sync": false,
        "active": false,
        "rev": -1,
        "defaultForRsProfiles": [],
    }));
    write_atomic(&dir.join("profiles.json"), &serde_json::to_vec(&root).unwrap())?;
    Ok(true)
}

/// Copy one profile's settings over the others named; each one replaced is kept as `.properties.bak`. Returns how
/// many were written. Only while no RuneLite runs.
pub fn share(dir: &Path, from: &str, to: &[String]) -> Result<usize, String> {
    let _lock = Lock::take(dir)?;
    let have = list(dir)?;
    let src = have.iter().find(|p| p.name == from).ok_or_else(|| format!("There is no profile called {from}."))?;
    let bytes = std::fs::read(file(dir, &src.name, src.id))
        .map_err(|_| format!("{from} has no settings saved yet; start RuneLite with it once first."))?;
    let mut n = 0;
    for name in to.iter().filter(|t| t.as_str() != from) {
        let Some(p) = have.iter().find(|p| &p.name == name) else { continue };
        let dest = file(dir, &p.name, p.id);
        if dest.exists() {
            let _ = std::fs::copy(&dest, dest.with_extension("properties.bak"));
        }
        write_atomic(&dest, &bytes)?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lodestone-profiles-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn seed(d: &Path) {
        std::fs::write(
            d.join("profiles.json"),
            r#"{"profiles":[{"id":-1,"name":"$rsprofile","sync":true,"active":false,"rev":-1,"defaultForRsProfiles":[]},
                {"id":438718536620607,"name":"default","sync":false,"active":true,"rev":-1,"defaultForRsProfiles":[]}],"extra":1}"#,
        )
        .unwrap();
        std::fs::write(d.join("default-438718536620607.properties"), "gpu.fps=50\n").unwrap();
    }

    #[test]
    fn makes_profiles_from_the_active_one() {
        let d = tmp("ensure");
        seed(&d);
        assert_eq!(list(&d).unwrap().iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["default"]);
        assert!(ensure(&d, "Iron Bob").unwrap());
        assert!(!ensure(&d, "Iron Bob").unwrap());
        let made = list(&d).unwrap().into_iter().find(|p| p.name == "Iron Bob").unwrap();
        assert!(!made.active && !made.sync);
        assert_eq!(std::fs::read_to_string(file(&d, "Iron Bob", made.id)).unwrap(), "gpu.fps=50\n");
        let root: Value = serde_json::from_slice(&std::fs::read(d.join("profiles.json")).unwrap()).unwrap();
        assert_eq!(root["extra"], 1);
        assert_eq!(root["profiles"].as_array().unwrap().len(), 3);
        assert!(ensure(&d, "../x").is_err() && ensure(&d, RS_PROFILE).is_err());
    }

    #[test]
    fn waits_for_runelite_before_its_first_start() {
        let d = tmp("fresh");
        assert!(!ensure(&d, "Bob").unwrap());
        assert!(!d.join("profiles.json").exists());
    }

    #[test]
    fn shares_settings_and_keeps_backups() {
        let d = tmp("share");
        seed(&d);
        ensure(&d, "Alt").unwrap();
        let alt = list(&d).unwrap().into_iter().find(|p| p.name == "Alt").unwrap();
        std::fs::write(file(&d, "Alt", alt.id), "gpu.fps=144\n").unwrap();
        std::fs::write(file(&d, "default", 438718536620607), "gpu.fps=30\n").unwrap();
        assert_eq!(share(&d, "default", &["Alt".into(), "default".into(), "Nobody".into()]).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(file(&d, "Alt", alt.id)).unwrap(), "gpu.fps=30\n");
        assert_eq!(std::fs::read_to_string(file(&d, "Alt", alt.id).with_extension("properties.bak")).unwrap(), "gpu.fps=144\n");
        assert!(share(&d, "Nobody", &[]).is_err());
    }

    #[test]
    fn picks_the_character_profile() {
        let mut s = Settings::default();
        let info = CharacterInfo { profile: "Shared".into(), ..Default::default() };
        assert_eq!(for_character(&s, None, "Bob"), None);
        assert_eq!(for_character(&s, Some(&info), "Bob").as_deref(), Some("Shared"));
        s.runelite_profiles = true;
        assert_eq!(for_character(&s, None, "Bob").as_deref(), Some("Bob"));
        assert_eq!(for_character(&s, None, ""), None);
    }
}
