//! The game clients: fetched from their official sources, checked, and kept up to date.
//! - RuneLite: the launcher jar from the latest GitHub release, verified against the release's SHA-256 digest.
//! - HDOS: the launcher jar from HDOS's CDN, re-downloaded only when the server says it changed.
//! - RuneScape 3: Jagex's Linux package, verified against the SHA-256 in their package index, then the game binary is
//!   unpacked from it. Nothing is installed system-wide.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;

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

fn load_meta(c: Client) -> Meta {
    std::fs::read(c.meta_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_meta(c: Client, m: &Meta) {
    let _ = std::fs::create_dir_all(c.dir());
    let _ = std::fs::write(c.meta_path(), serde_json::to_vec_pretty(m).unwrap());
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
    pub previous_version: String,
}

pub fn status(c: Client) -> Status {
    let m = load_meta(c);
    let size = std::fs::metadata(c.file()).map(|m| m.len()).unwrap_or(0);
    Status {
        client: c,
        installed: size > 0,
        version: m.version,
        checked_at: m.checked_at,
        installed_at: m.installed_at,
        size,
        last_error: m.last_error,
        previous_version: m.previous_version,
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
    if st.installed && !force && now().saturating_sub(st.checked_at) < FRESH_SECS {
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
    let dest = c.file();
    let (sha, _) = save(c, res, &dest, emit).await?;
    if let Some(want) = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")) {
        if !want.eq_ignore_ascii_case(&sha) {
            return Err(mismatch(&dest));
        }
    }
    save_meta(c, &Meta::fresh(rel.tag_name, sha).after(meta));
    Ok(())
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
    let (sha, _) = save(c, res, &c.file(), emit).await?;
    let version = if last_modified.is_empty() { format!("sha256 {}", &sha[..12]) } else { last_modified.clone() };
    save_meta(c, &Meta { etag, last_modified, ..Meta::fresh(version, sha).after(meta) });
    Ok(())
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
    let dest = c.file();
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
    save_meta(c, &Meta::fresh(pkg.version, sha).after(meta));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
