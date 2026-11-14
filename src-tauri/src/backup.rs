//! Backups of what took time to set up: Lodestone's settings, groups, character notes, window spots and play time,
//! and RuneLite's settings profiles. One .tar.gz per backup in Documents/VOID Lodestone backups. Never in one: sign-in
//! sessions, the keyring, RuneLite's credentials.properties or anything else outside the few files named below.
//! Also brings RuneLite profiles over from Bolt (or any other .runelite folder), reading only the same files.

use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::config::{self, Account, Store};
use crate::playtime;

const CONFIG: &str = "lodestone/config.json";
const PLAYTIME: &str = "lodestone/playtime.json";
const RL_SETTINGS: &str = "runelite/settings.properties";
const PROFILES: &str = "runelite/profiles2/";
/// More than any real backup; stops a damaged or hostile archive from filling the disk.
const MAX: u64 = 64 << 20;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub name: String,
    pub size: u64,
    pub created: u64,
}

/// What a backup (or another .runelite folder) holds, read and checked.
#[derive(Debug, Default)]
pub struct Contents {
    pub store: Option<Store>,
    pub playtime: Option<playtime::Book>,
    pub runelite_settings: Option<Vec<u8>>,
    /// profiles2's files by name.
    pub profiles: Vec<(String, Vec<u8>)>,
}

pub fn dir() -> PathBuf {
    documents().join("VOID Lodestone backups")
}

/// The Documents folder from user-dirs.dirs (the Flatpak keeps a copy in the app's config folder), else ~/Documents.
pub fn documents() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute());
    for f in xdg.into_iter().chain([home.join(".config")]).map(|d| d.join("user-dirs.dirs")) {
        let Ok(s) = std::fs::read_to_string(f) else { continue };
        if let Some(p) = s.lines().find_map(|l| documents_line(l, &home)) {
            return p;
        }
    }
    home.join("Documents")
}

fn documents_line(line: &str, home: &Path) -> Option<PathBuf> {
    let v = line.trim().strip_prefix("XDG_DOCUMENTS_DIR=")?.trim_matches('"');
    let p = match v.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(v),
    };
    (p.is_absolute() && p != home).then_some(p)
}

/// Local time as 2026-11-14-093005, for file names.
pub fn stamp(t: u64) -> String {
    // SAFETY: localtime_r only writes the tm it's given.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let t = t as libc::time_t;
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!("{:04}-{:02}-{:02}-{:02}{:02}{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
}

/// The profiles2 files worth keeping: the list and each profile's settings. Not the lock, backups or temp files.
fn profile_file(name: &str) -> bool {
    !name.starts_with('.') && !name.contains(['/', '\\', '\0']) && (name == "profiles.json" || name.ends_with(".properties"))
}

fn read_limited(path: &Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("couldn't read {}: {e}", path.display()))?;
    if !meta.is_file() || meta.len() > MAX {
        return Err(format!("{} isn't a settings file", path.display()));
    }
    std::fs::read(path).map_err(|e| format!("couldn't read {}: {e}", path.display()))
}

/// A .runelite folder's settings and profiles (nothing else in it is read).
pub fn read_runelite(runelite: &Path) -> Result<Contents, String> {
    let mut c = Contents::default();
    let settings = runelite.join("settings.properties");
    if settings.is_file() {
        c.runelite_settings = Some(read_limited(&settings)?);
    }
    let mut total = 0;
    if let Ok(rd) = std::fs::read_dir(runelite.join("profiles2")) {
        for e in rd.flatten() {
            let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
            if !profile_file(&name) || !e.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let bytes = read_limited(&e.path())?;
            total += bytes.len() as u64;
            if total > MAX {
                return Err("RuneLite's profiles are too big to copy".into());
            }
            c.profiles.push((name, bytes));
        }
    }
    c.profiles.sort();
    Ok(c)
}

/// Write a new backup; returns its file name.
pub fn create(store: &Store, runelite: &Path) -> Result<String, String> {
    let d = dir();
    std::fs::create_dir_all(&d).map_err(|e| format!("couldn't create {}: {e}", d.display()))?;
    let now = config::now();
    let mut name = format!("lodestone-backup-{}.tar.gz", stamp(now));
    let mut n = 1;
    while d.join(&name).exists() {
        n += 1;
        name = format!("lodestone-backup-{}-{n}.tar.gz", stamp(now));
    }
    let rl = read_runelite(runelite)?;
    let tmp = d.join(format!(".{name}.tmp"));
    let res = (|| -> std::io::Result<()> {
        let f = config::private_file(&tmp)?;
        let mut tar = tar::Builder::new(GzEncoder::new(f, Compression::default()));
        let mut add = |path: &str, bytes: &[u8]| -> std::io::Result<()> {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(tar::EntryType::Regular);
            h.set_size(bytes.len() as u64);
            h.set_mode(0o600);
            h.set_mtime(now);
            h.set_cksum();
            tar.append_data(&mut h, path, bytes)
        };
        add(CONFIG, &serde_json::to_vec_pretty(store).expect("settings serialise"))?;
        let book = playtime::load(&playtime::path());
        add(PLAYTIME, &serde_json::to_vec(&book).expect("play time serialises"))?;
        if let Some(s) = &rl.runelite_settings {
            add(RL_SETTINGS, s)?;
        }
        for (file, bytes) in &rl.profiles {
            add(&format!("{PROFILES}{file}"), bytes)?;
        }
        tar.into_inner()?.finish()?.sync_all()
    })();
    match res.and_then(|_| std::fs::rename(&tmp, d.join(&name))) {
        Ok(()) => Ok(name),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("couldn't write the backup: {e}"))
        }
    }
}

/// The backups in the folder, newest first.
pub fn list() -> Vec<Backup> {
    let mut out: Vec<Backup> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let meta = e.metadata().ok()?;
            (name.ends_with(".tar.gz") && !name.starts_with('.') && meta.is_file()).then(|| Backup {
                created: meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()),
                size: meta.len(),
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then(b.name.cmp(&a.name)));
    out
}

/// A backup in the folder by its file name (no paths).
pub fn path_of(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\', '\0']) || !name.ends_with(".tar.gz") {
        return Err("That isn't a Lodestone backup.".into());
    }
    Ok(dir().join(name))
}

/// Read a backup, taking only the files a backup can hold; anything else in the archive is ignored.
pub fn read(path: &Path) -> Result<Contents, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("couldn't open the backup: {e}"))?;
    let mut tar = tar::Archive::new(GzDecoder::new(f));
    let mut c = Contents::default();
    let mut total = 0;
    let bad = |e: std::io::Error| format!("the backup is damaged: {e}");
    for entry in tar.entries().map_err(bad)? {
        let mut entry = entry.map_err(bad)?;
        if entry.header().entry_type() != tar::EntryType::Regular {
            continue;
        }
        let Some(path) = entry.path().ok().and_then(|p| p.to_str().map(str::to_string)) else { continue };
        let size = entry.header().size().map_err(bad)?;
        total += size;
        if size > MAX || total > MAX {
            return Err("the backup is too big to be one of Lodestone's".into());
        }
        let wanted = path == CONFIG
            || path == PLAYTIME
            || path == RL_SETTINGS
            || path.strip_prefix(PROFILES).is_some_and(profile_file);
        if !wanted {
            continue;
        }
        let mut bytes = Vec::with_capacity(size as usize);
        (&mut entry).take(MAX).read_to_end(&mut bytes).map_err(bad)?;
        match path.as_str() {
            CONFIG => c.store = Some(serde_json::from_slice(&bytes).map_err(|e| format!("the backup's settings are damaged: {e}"))?),
            PLAYTIME => c.playtime = serde_json::from_slice(&bytes).ok(),
            RL_SETTINGS => c.runelite_settings = Some(bytes),
            p => c.profiles.push((p[PROFILES.len()..].to_string(), bytes)),
        }
    }
    if c.store.is_none() && c.profiles.is_empty() {
        return Err("That file isn't a Lodestone backup.".into());
    }
    c.profiles.sort();
    Ok(c)
}

/// Put RuneLite's profiles (and its settings.properties) in place. What was there is kept beside it as
/// `profiles2.before-restore-<time>`. Only while no RuneLite runs.
pub fn put_runelite(runelite: &Path, c: &Contents) -> Result<usize, String> {
    if c.profiles.is_empty() && c.runelite_settings.is_none() {
        return Ok(0);
    }
    if !c.profiles.is_empty() && !c.profiles.iter().any(|(n, _)| n == "profiles.json") {
        return Err("The profiles have no profiles.json, so RuneLite couldn't use them.".into());
    }
    let s = stamp(config::now());
    std::fs::create_dir_all(runelite).map_err(|e| format!("couldn't create {}: {e}", runelite.display()))?;
    let io = |e: std::io::Error| format!("couldn't put RuneLite's settings in place: {e}");
    if let Some(bytes) = &c.runelite_settings {
        let f = runelite.join("settings.properties");
        if f.exists() {
            std::fs::copy(&f, runelite.join(format!("settings.properties.before-restore-{s}"))).map_err(io)?;
        }
        std::fs::write(&f, bytes).map_err(io)?;
    }
    if !c.profiles.is_empty() {
        let dest = runelite.join("profiles2");
        if dest.exists() {
            std::fs::rename(&dest, runelite.join(format!("profiles2.before-restore-{s}"))).map_err(io)?;
        }
        std::fs::create_dir_all(&dest).map_err(io)?;
        for (name, bytes) in &c.profiles {
            std::fs::write(dest.join(name), bytes).map_err(io)?;
        }
    }
    Ok(c.profiles.iter().filter(|(n, _)| n.ends_with(".properties")).count())
}

/// Take a backup's settings, groups, notes and window spots. Accounts already here stay as they are (their sign-in
/// is in the keyring); ones that aren't are added and need signing in. Picks made under another id for the same Jagex
/// account follow it. Returns the old→new account ids used.
pub fn merge(cur: &mut Store, from: Store) -> HashMap<String, String> {
    let mut ids = HashMap::new();
    for a in from.accounts {
        match cur.accounts.iter().find(|c| c.id == a.id || (!a.subject.is_empty() && c.subject == a.subject)) {
            Some(have) => {
                ids.insert(a.id.clone(), have.id.clone());
            }
            None => cur.accounts.push(Account { signed_in_at: 0, ..a }),
        }
    }
    let seen = std::mem::take(&mut cur.settings.seen_version);
    cur.settings = from.settings;
    cur.settings.seen_version = seen;
    for g in &mut cur.settings.groups {
        for r in &mut g.rows {
            r.pick = remap(&ids, &r.pick);
        }
    }
    cur.settings.last_character = remap(&ids, &cur.settings.last_character);
    cur.characters = from.characters.into_iter().map(|(k, v)| (remap(&ids, &k), v)).collect();
    cur.windows = from.windows.into_iter().map(|(k, v)| (remap(&ids, &k), v)).collect();
    ids
}

/// `old account:character` → `new account:character`.
pub fn remap(ids: &HashMap<String, String>, pick: &str) -> String {
    match pick.split_once(':') {
        Some((a, c)) if ids.contains_key(a) => format!("{}:{c}", ids[a]),
        _ => pick.to_string(),
    }
}

/// Where Bolt's RuneLite keeps its folder inside what the user points at: the folder itself, or a Bolt data folder.
pub fn find_runelite(dir: &Path) -> Option<PathBuf> {
    [
        dir.to_path_buf(),
        dir.join(".runelite"),
        dir.join("bolt-launcher/.runelite"),
        dir.join("data/bolt-launcher/.runelite"),
    ]
    .into_iter()
    .find(|d| d.join("profiles2/profiles.json").is_file())
}

/// Bolt's RuneLite folders this can see: its Flatpak and native data folders, and a copy put in Documents (the only
/// one the Lodestone Flatpak can read).
pub fn bolt_folders() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let mut out: Vec<PathBuf> = [
        home.join(".var/app/com.adamcake.Bolt/data/bolt-launcher"),
        home.join(".local/share/bolt-launcher"),
        documents(),
        documents().join("Bolt"),
    ]
    .iter()
    .filter_map(|d| find_runelite(d))
    .collect();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CharacterInfo, Group, GroupRow, WindowSpot};
    use crate::jagex::Character;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lodestone-backup-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn account(id: &str, sub: &str) -> Account {
        Account {
            id: id.into(),
            label: id.into(),
            subject: sub.into(),
            characters: vec![Character { account_id: "c1".into(), display_name: "Zezima".into() }],
            added_at: 1,
            signed_in_at: 2,
        }
    }

    #[test]
    fn reads_documents_from_user_dirs() {
        let home = Path::new("/home/me");
        assert_eq!(documents_line(r#"XDG_DOCUMENTS_DIR="$HOME/Docs""#, home), Some(PathBuf::from("/home/me/Docs")));
        assert_eq!(documents_line(r#"XDG_DOCUMENTS_DIR="/data/docs""#, home), Some(PathBuf::from("/data/docs")));
        assert_eq!(documents_line(r#"XDG_DOCUMENTS_DIR="$HOME/""#, home), None);
        assert_eq!(documents_line(r#"XDG_MUSIC_DIR="$HOME/Music""#, home), None);
    }

    #[test]
    fn copies_only_settings_from_runelite() {
        let rl = tmp("rl");
        std::fs::create_dir_all(rl.join("profiles2")).unwrap();
        for f in ["profiles2/profiles.json", "profiles2/default-1.properties", "profiles2/profiles.lck", "profiles2/x.properties.bak"] {
            std::fs::write(rl.join(f), f).unwrap();
        }
        std::fs::write(rl.join("credentials.properties"), "secret").unwrap();
        std::fs::write(rl.join("settings.properties"), "old=1").unwrap();
        let c = read_runelite(&rl).unwrap();
        let names: Vec<_> = c.profiles.iter().map(|p| p.0.as_str()).collect();
        assert_eq!(names, ["default-1.properties", "profiles.json"]);
        assert_eq!(c.runelite_settings.as_deref(), Some(&b"old=1"[..]));
        assert_eq!(find_runelite(&rl), Some(rl.clone()));
        assert_eq!(find_runelite(rl.parent().unwrap().join("nowhere").as_path()), None);

        let into = tmp("into");
        std::fs::create_dir_all(into.join("profiles2")).unwrap();
        std::fs::write(into.join("profiles2/mine.properties"), "mine").unwrap();
        std::fs::write(into.join("settings.properties"), "mine=1").unwrap();
        assert_eq!(put_runelite(&into, &c).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(into.join("profiles2/default-1.properties")).unwrap(), "profiles2/default-1.properties");
        assert!(!into.join("profiles2/mine.properties").exists());
        let kept = std::fs::read_dir(&into).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains("before-restore")).count();
        assert_eq!(kept, 2);
        assert!(!into.join("credentials.properties").exists());
    }

    #[test]
    fn reads_only_known_files_from_an_archive() {
        let d = tmp("archive");
        let p = d.join("x.tar.gz");
        let mut tar = tar::Builder::new(GzEncoder::new(std::fs::File::create(&p).unwrap(), Compression::default()));
        // Names written as they are, as a hostile archive would, past the checks set_path makes.
        let mut add = |path: &str, bytes: &[u8]| {
            let mut h = tar::Header::new_gnu();
            h.as_gnu_mut().unwrap().name[..path.len()].copy_from_slice(path.as_bytes());
            h.set_size(bytes.len() as u64);
            h.set_mode(0o600);
            h.set_entry_type(tar::EntryType::Regular);
            h.set_cksum();
            tar.append(&h, bytes).unwrap();
        };
        add(CONFIG, br#"{"settings":{"theme":"abyss"}}"#);
        add("runelite/profiles2/profiles.json", b"{}");
        add("runelite/profiles2/../../evil.properties", b"x");
        add("runelite/credentials.properties", b"x");
        add("lodestone/other", b"x");
        tar.into_inner().unwrap().finish().unwrap();
        let c = read(&p).unwrap();
        assert_eq!(c.store.unwrap().settings.theme, "abyss");
        assert_eq!(c.profiles.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(), ["profiles.json"]);
        assert!(path_of("../x.tar.gz").is_err() && path_of("a.tar.gz").is_ok());
    }

    #[test]
    fn merges_keeping_signed_in_accounts() {
        let mut cur = Store::default();
        cur.accounts.push(account("new", "sub-a"));
        cur.settings.seen_version = "0.4.0".into();
        let mut from = Store::default();
        from.accounts = vec![account("old", "sub-a"), account("other", "sub-b")];
        from.settings.theme = "abyss".into();
        from.settings.groups = vec![Group { name: "g".into(), game: "osrs".into(), rows: vec![GroupRow { pick: "old:c1".into(), ..Default::default() }] }];
        from.characters.insert("old:c1".into(), CharacterInfo { notes: "quest".into(), ..Default::default() });
        from.windows.insert("other:c1".into(), WindowSpot { x: 5, ..Default::default() });
        merge(&mut cur, from);
        assert_eq!(cur.accounts.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["new", "other"]);
        assert_eq!(cur.accounts[1].signed_in_at, 0);
        assert_eq!(cur.settings.theme, "abyss");
        assert_eq!(cur.settings.seen_version, "0.4.0");
        assert_eq!(cur.settings.groups[0].rows[0].pick, "new:c1");
        assert_eq!(cur.characters["new:c1"].notes, "quest");
        assert_eq!(cur.windows["other:c1"].x, 5);
    }
}
