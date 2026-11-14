//! The game clients: fetched from their official sources, checked, and kept up to date.
//! - RuneLite: the launcher jar from the latest GitHub release, verified against the release's SHA-256 digest.
//! - HDOS: the launcher jar from HDOS's CDN, re-downloaded only when the server says it changed.
//! - RuneScape 3: Jagex's Linux package, verified against the SHA-256 in their package index, then the game binary is
//!   unpacked from it. Nothing is installed system-wide.
//!
//! Each update is downloaded beside the client and checked before it replaces it; the version it replaces is kept
//! (`<file>.previous`) so a broken update can be rolled back. A rolled-back client stays pinned until updates resume.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::config::{data_dir, now};

pub const RUNELITE_RELEASE: &str = "https://api.github.com/repos/runelite/launcher/releases/latest";
pub const HDOS_JAR: &str = "https://cdn.hdos.dev/launcher/v8/hdos-launcher.jar";
pub const RS3_REPO: &str = "https://content.runescape.com/downloads/ubuntu/";
pub const RS3_PACKAGES: &str =
    "https://content.runescape.com/downloads/ubuntu/dists/trusty/non-free/binary-amd64/Packages";
const RS3_BINARY: &str = "usr/share/games/runescape-launcher/runescape";
/// How long a check stays fresh before launching checks again.
const FRESH_SECS: u64 = 6 * 3600;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    Runelite,
    Hdos,
    Rs3,
}

impl Client {
    pub const ALL: [Client; 3] = [Client::Runelite, Client::Hdos, Client::Rs3];

    pub fn id(self) -> &'static str {
        match self {
            Client::Runelite => "runelite",
            Client::Hdos => "hdos",
            Client::Rs3 => "rs3",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Client::Runelite => "RuneLite",
            Client::Hdos => "HDOS",
            Client::Rs3 => "RuneScape 3",
        }
    }

    pub fn dir(self) -> PathBuf {
        data_dir().join("clients").join(self.id())
    }

    pub fn file(self) -> PathBuf {
        self.dir().join(match self {
            Client::Runelite => "RuneLite.jar",
            Client::Hdos => "hdos-launcher.jar",
            Client::Rs3 => "runescape",
        })
    }

    fn meta_path(self) -> PathBuf {
        self.dir().join("meta.json")
    }

    /// Where a download waits until it's checked.
    fn staged(self) -> PathBuf {
        with_suffix(&self.file(), ".new")
    }

    fn slot(self) -> Slot {
        Slot::new(self.dir(), self.file())
    }
}

/// A client's file, the version kept to roll back to, and what's known about each.
struct Slot {
    file: PathBuf,
    meta: PathBuf,
    previous: PathBuf,
    previous_meta: PathBuf,
}

impl Slot {
    fn new(dir: PathBuf, file: PathBuf) -> Self {
        Slot { previous: with_suffix(&file, ".previous"), meta: dir.join("meta.json"), previous_meta: dir.join("meta.previous.json"), file }
    }
}

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Meta {
    version: String,
    etag: String,
    last_modified: String,
    sha256: String,
    checked_at: u64,
    installed_at: u64,
    /// Why the last update check failed; empty when it worked.
    last_error: String,
    /// The version this one replaced.
    previous_version: String,
    /// Rolled back: kept at this version, with no update checks, until updates resume.
    pinned: bool,
}

impl Meta {
    fn fresh(version: String, sha256: String) -> Self {
        Meta { version, sha256, checked_at: now(), installed_at: now(), ..Meta::default() }
    }

    /// Remember what this download replaced (a re-download of the same version keeps the older note).
    fn after(self, old: Meta) -> Self {
        let previous_version = if old.version == self.version { old.previous_version } else { old.version };
        Meta { previous_version, ..self }
    }
}

fn read_meta(p: &Path) -> Option<Meta> {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

fn load_meta(c: Client) -> Meta {
    read_meta(&c.meta_path()).unwrap_or_default()
}

fn write_meta(p: &Path, m: &Meta) {
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, serde_json::to_vec_pretty(m).unwrap());
}

fn save_meta(c: Client, m: &Meta) {
    write_meta(&c.meta_path(), m);
}

/// Put a checked download in place. A different version than the installed one moves that aside as the one to roll
/// back to; the same version again (a repaired download) just replaces it.
fn put_in_place(c: Client, staged: &Path, new: Meta, old: Meta) -> Result<(), String> {
    put_in_slot(&c.slot(), staged, new, old)
}

fn put_in_slot(s: &Slot, staged: &Path, new: Meta, old: Meta) -> Result<(), String> {
    if !old.version.is_empty() && old.version != new.version && s.file.exists() {
        std::fs::rename(&s.file, &s.previous).map_err(|e| format!("couldn't keep the old version: {e}"))?;
        write_meta(&s.previous_meta, &Meta { pinned: false, ..old.clone() });
    }
    std::fs::rename(staged, &s.file).map_err(|e| format!("couldn't install the update: {e}"))?;
    write_meta(&s.meta, &new.after(old));
    Ok(())
}

/// Go back to the version the last update replaced, and stay there. Swaps the two, so rolling back again undoes it.
pub fn roll_back(c: Client) -> Result<Status, String> {
    roll_back_slot(&c.slot())?;
    Ok(status(c))
}

fn roll_back_slot(s: &Slot) -> Result<(), String> {
    let prev_meta = read_meta(&s.previous_meta).filter(|_| s.previous.is_file()).ok_or("There is no earlier version kept.")?;
    let cur_meta = read_meta(&s.meta).unwrap_or_default();
    let swap = with_suffix(&s.file, ".swap");
    std::fs::rename(&s.file, &swap)
        .and_then(|_| std::fs::rename(&s.previous, &s.file))
        .and_then(|_| std::fs::rename(&swap, &s.previous))
        .map_err(|e| format!("couldn't swap the versions: {e}"))?;
    write_meta(&s.previous_meta, &Meta { pinned: false, ..cur_meta.clone() });
    let previous_version = cur_meta.version;
    let blank = (String::new(), String::new(), String::new());
    write_meta(&s.meta, &Meta { pinned: true, etag: blank.0, last_modified: blank.1, last_error: blank.2, previous_version, ..prev_meta });
    Ok(())
}

/// Unpin a rolled-back client; the next check (made now by the caller) updates it.
pub fn resume(c: Client) {
    let m = load_meta(c);
    save_meta(c, &Meta { pinned: false, checked_at: 0, ..m });
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub client: Client,
    pub installed: bool,
    pub version: String,
    pub checked_at: u64,
    pub installed_at: u64,
    pub size: u64,
    pub last_error: String,
    /// The version to roll back to when `can_roll_back`, else the one the installed copy replaced.
    pub previous_version: String,
    pub pinned: bool,
    pub can_roll_back: bool,
}

pub fn status(c: Client) -> Status {
    let m = load_meta(c);
    let size = std::fs::metadata(c.file()).map(|m| m.len()).unwrap_or(0);
    let slot = c.slot();
    let prev = read_meta(&slot.previous_meta).filter(|_| slot.previous.is_file());
    Status {
        client: c,
        installed: size > 0,
        version: m.version,
        checked_at: m.checked_at,
        installed_at: m.installed_at,
        size,
        last_error: m.last_error,
        can_roll_back: size > 0 && prev.is_some(),
        previous_version: prev.map_or(m.previous_version, |p| p.version),
        pinned: m.pinned,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub client: Client,
    /// checking | downloading | unpacking | ready | error
    pub phase: &'static str,
    pub received: u64,
    pub total: u64,
    pub message: String,
}

impl Progress {
    fn new(client: Client, phase: &'static str) -> Self {
        Self { client, phase, received: 0, total: 0, message: String::new() }
    }
}

/// Make sure a client is present and, if its last check is stale (or `force`), up to date. A failed check with a
/// client already installed keeps the installed copy rather than blocking the launch.
pub async fn ensure(
    http: &reqwest::Client,
    c: Client,
    force: bool,
    emit: &(dyn Fn(Progress) + Send + Sync),
) -> Result<Status, String> {
    let st = status(c);
    if st.installed && (st.pinned || !force && now().saturating_sub(st.checked_at) < FRESH_SECS) {
        return Ok(st);
    }
    emit(Progress::new(c, "checking"));
    std::fs::create_dir_all(c.dir()).map_err(|e| format!("couldn't create {}: {e}", c.dir().display()))?;
    let res = match c {
        Client::Runelite => update_runelite(http, emit).await,
        Client::Hdos => update_hdos(http, emit).await,
        Client::Rs3 => update_rs3(http, emit).await,
    };
    let mut meta = load_meta(c);
    let error = res.as_ref().err().cloned().unwrap_or_default();
    if meta.last_error != error {
        meta.last_error = error;
        save_meta(c, &meta);
    }
    match res {
        Ok(()) => {
            let st = status(c);
            emit(Progress { message: st.version.clone(), ..Progress::new(c, "ready") });
            Ok(st)
        }
        Err(e) => {
            emit(Progress { message: e.clone(), ..Progress::new(c, "error") });
            if st.installed {
                Ok(st)
            } else {
                Err(format!("couldn't download {}: {e}", c.name()))
            }
        }
    }
}

fn touch(c: Client, mut m: Meta) {
    m.checked_at = now();
    save_meta(c, &m);
}

/// Stream a response into `dest` (via a temp file), hashing as it goes. Returns the SHA-256 hex digest and size.
async fn save(
    c: Client,
    mut res: reqwest::Response,
    dest: &PathBuf,
    emit: &(dyn Fn(Progress) + Send + Sync),
) -> Result<(String, u64), String> {
    if !res.status().is_success() {
        return Err(format!("the server returned {}", res.status()));
    }
    let total = res.content_length().unwrap_or(0);
    let tmp = dest.with_extension("part");
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let (mut received, mut shown) = (0u64, 0u64);
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("download interrupted: {e}"))? {
        f.write_all(&chunk).map_err(|e| e.to_string())?;
        hash.update(&chunk);
        received += chunk.len() as u64;
        if received - shown >= 256 * 1024 {
            shown = received;
            emit(Progress { received, total, ..Progress::new(c, "downloading") });
        }
    }
    f.sync_all().map_err(|e| e.to_string())?;
    drop(f);
    if received == 0 {
        let _ = std::fs::remove_file(&tmp);
        return Err("the download was empty".into());
    }
    emit(Progress { received, total: total.max(received), ..Progress::new(c, "downloading") });
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    Ok((hex(&hash.finalize()), received))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn mismatch(dest: &PathBuf) -> String {
    let _ = std::fs::remove_file(dest);
    "the download didn't match its published checksum, so it was discarded".into()
}

async fn update_runelite(http: &reqwest::Client, emit: &(dyn Fn(Progress) + Send + Sync)) -> Result<(), String> {
    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
        assets: Vec<Asset>,
    }
    #[derive(Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
        #[serde(default)]
        digest: Option<String>,
    }
    let c = Client::Runelite;
    let rel: Release = http
        .get(RUNELITE_RELEASE)
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("couldn't check for RuneLite updates: {e}"))?
        .json()
        .await
        .map_err(|e| format!("unexpected reply from GitHub: {e}"))?;
    let meta = load_meta(c);
    if meta.version == rel.tag_name && status(c).installed {
        touch(c, meta);
        return Ok(());
    }
    let asset = rel.assets.iter().find(|a| a.name == "RuneLite.jar").ok_or("the release has no RuneLite.jar")?;
    let res = http.get(&asset.browser_download_url).send().await.map_err(|e| e.to_string())?;
    let staged = c.staged();
    let (sha, _) = save(c, res, &staged, emit).await?;
    if let Some(want) = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")) {
        if !want.eq_ignore_ascii_case(&sha) {
            return Err(mismatch(&staged));
        }
    }
    put_in_place(c, &staged, Meta::fresh(rel.tag_name, sha), meta)
}

async fn update_hdos(http: &reqwest::Client, emit: &(dyn Fn(Progress) + Send + Sync)) -> Result<(), String> {
    let c = Client::Hdos;
    let meta = load_meta(c);
    let mut req = http.get(HDOS_JAR);
    if status(c).installed {
        if !meta.etag.is_empty() {
            req = req.header("If-None-Match", &meta.etag);
        }
        if !meta.last_modified.is_empty() {
            req = req.header("If-Modified-Since", &meta.last_modified);
        }
    }
    let res = req.send().await.map_err(|e| format!("couldn't reach HDOS: {e}"))?;
    if res.status() == reqwest::StatusCode::NOT_MODIFIED {
        touch(c, meta);
        return Ok(());
    }
    let header = |k: &str| res.headers().get(k).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let (etag, last_modified) = (header("etag"), header("last-modified"));
    let staged = c.staged();
    let (sha, _) = save(c, res, &staged, emit).await?;
    let version = if last_modified.is_empty() { format!("sha256 {}", &sha[..12]) } else { last_modified.clone() };
    put_in_place(c, &staged, Meta { etag, last_modified, ..Meta::fresh(version, sha) }, meta)
}

#[derive(Debug, PartialEq)]
pub struct Package {
    pub version: String,
    pub filename: String,
    pub sha256: String,
}

/// Pick the runescape-launcher entry out of a Debian `Packages` index.
pub fn parse_packages(text: &str) -> Option<Package> {
    text.split("\n\n")
        .filter_map(|stanza| {
            let field = |k: &str| {
                stanza.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix(':')).map(|v| v.trim().to_string()))
            };
            (field("Package")? == "runescape-launcher").then_some(())?;
            Some(Package { version: field("Version")?, filename: field("Filename")?, sha256: field("SHA256")? })
        })
        .last()
}

/// Pull the game binary out of the .deb (an `ar` archive holding data.tar.xz).
pub fn extract_rs3(deb: &[u8]) -> Result<Vec<u8>, String> {
    let mut ar = ar::Archive::new(deb);
    while let Some(entry) = ar.next_entry() {
        let mut entry = entry.map_err(|e| format!("bad package: {e}"))?;
        let name = String::from_utf8_lossy(entry.header().identifier()).to_string();
        if !name.starts_with("data.tar") {
            continue;
        }
        let mut packed = Vec::new();
        entry.read_to_end(&mut packed).map_err(|e| e.to_string())?;
        let tar_bytes = if name.ends_with(".xz") {
            let mut out = Vec::new();
            lzma_rs::xz_decompress(&mut &packed[..], &mut out).map_err(|e| format!("bad package data: {e}"))?;
            out
        } else if name == "data.tar" {
            packed
        } else {
            return Err(format!("unsupported package compression ({name})"));
        };
        let mut tar = tar::Archive::new(&tar_bytes[..]);
        for file in tar.entries().map_err(|e| e.to_string())? {
            let mut file = file.map_err(|e| e.to_string())?;
            let path = file.path().map_err(|e| e.to_string())?.to_string_lossy().trim_start_matches("./").to_string();
            if path == RS3_BINARY {
                let mut out = Vec::new();
                file.read_to_end(&mut out).map_err(|e| e.to_string())?;
                return Ok(out);
            }
        }
        return Err("the package doesn't contain the RuneScape binary".into());
    }
    Err("the package has no data archive".into())
}

async fn update_rs3(http: &reqwest::Client, emit: &(dyn Fn(Progress) + Send + Sync)) -> Result<(), String> {
    let c = Client::Rs3;
    let index = http
        .get(RS3_PACKAGES)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("couldn't check for RuneScape updates: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let pkg = parse_packages(&index).ok_or("Jagex's package index has no runescape-launcher")?;
    let meta = load_meta(c);
    if meta.version == pkg.version && status(c).installed {
        touch(c, meta);
        return Ok(());
    }
    if pkg.filename.contains("..") || pkg.filename.starts_with('/') {
        return Err("Jagex's package index has an unexpected file name".into());
    }
    let deb = c.dir().join("runescape-launcher.deb");
    let res = http.get(format!("{RS3_REPO}{}", pkg.filename)).send().await.map_err(|e| e.to_string())?;
    let (sha, _) = save(c, res, &deb, emit).await?;
    if !pkg.sha256.eq_ignore_ascii_case(&sha) {
        return Err(mismatch(&deb));
    }
    emit(Progress::new(c, "unpacking"));
    let dest = c.staged();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let bytes = std::fs::read(&deb).map_err(|e| e.to_string())?;
        let bin = extract_rs3(&bytes)?;
        let tmp = dest.with_extension("part");
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o755)
                .open(&tmp)
                .map_err(|e| e.to_string())?;
            f.write_all(&bin).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&deb);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    put_in_place(c, &c.staged(), Meta::fresh(pkg.version, sha), meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_old_version_to_roll_back_to() {
        let d = std::env::temp_dir().join(format!("lodestone-clients-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let s = Slot::new(d.clone(), d.join("RuneLite.jar"));
        let staged = d.join("RuneLite.jar.new");
        let install = |body: &str, version: &str| {
            std::fs::write(&staged, body).unwrap();
            let old = read_meta(&s.meta).unwrap_or_default();
            put_in_slot(&s, &staged, Meta::fresh(version.into(), body.into()), old).unwrap();
        };
        install("one", "2.7.0");
        assert!(!s.previous.exists());
        assert!(roll_back_slot(&s).is_err());
        install("one again", "2.7.0");
        assert!(!s.previous.exists());
        install("two", "2.7.1");
        assert_eq!(std::fs::read_to_string(&s.previous).unwrap(), "one again");

        roll_back_slot(&s).unwrap();
        let m = read_meta(&s.meta).unwrap();
        assert_eq!((m.version.as_str(), m.pinned, m.previous_version.as_str()), ("2.7.0", true, "2.7.1"));
        assert_eq!(std::fs::read_to_string(&s.file).unwrap(), "one again");
        assert_eq!(read_meta(&s.previous_meta).unwrap().version, "2.7.1");

        roll_back_slot(&s).unwrap();
        assert_eq!(std::fs::read_to_string(&s.file).unwrap(), "two");
        assert_eq!(read_meta(&s.meta).unwrap().version, "2.7.1");
    }

    #[test]
    fn parses_the_package_index() {
        let idx = "Package: other\nVersion: 1\nFilename: x\nSHA256: aa\n\n\
                   Package: runescape-launcher\nVersion: 2.2.12\nArchitecture: amd64\n\
                   Filename: pool/non-free/r/runescape-launcher/runescape-launcher_2.2.12_amd64.deb\nSize: 1\n\
                   SHA256: 676df79243a10bacf4069ec00a9225cdf65b5dea5939489b39f8fd0fa03b1394\n";
        let p = parse_packages(idx).unwrap();
        assert_eq!(p.version, "2.2.12");
        assert!(p.filename.ends_with("_amd64.deb"));
        assert!(p.sha256.starts_with("676df7"));
        assert!(parse_packages("Package: other\nVersion: 1\n").is_none());
    }

    fn fake_deb(path: &str, body: &[u8]) -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_gnu();
        h.set_size(body.len() as u64);
        h.set_mode(0o755);
        h.set_cksum();
        tar.append_data(&mut h, path, body).unwrap();
        let tar = tar.into_inner().unwrap();
        let mut xz = Vec::new();
        lzma_rs::xz_compress(&mut &tar[..], &mut xz).unwrap();
        let mut deb = ar::Builder::new(Vec::new());
        deb.append(&ar::Header::new(b"debian-binary".to_vec(), 4), &b"2.0\n"[..]).unwrap();
        deb.append(&ar::Header::new(b"data.tar.xz".to_vec(), xz.len() as u64), &xz[..]).unwrap();
        deb.into_inner().unwrap()
    }

    #[test]
    fn extracts_the_binary_from_a_deb() {
        let deb = fake_deb("./usr/share/games/runescape-launcher/runescape", b"\x7fELF-ish");
        assert_eq!(extract_rs3(&deb).unwrap(), b"\x7fELF-ish");
        let other = fake_deb("./usr/bin/something", b"x");
        assert!(extract_rs3(&other).is_err());
    }
}
