//! Lodestone's own updates, from the project's GitHub releases. Inside the Flatpak the release's bundle is downloaded,
//! checked against GitHub's published sha256 and installed with the host's `flatpak` (through `flatpak-spawn --host`,
//! which needs the org.freedesktop.Flatpak bus name). Elsewhere the release page is opened instead.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;

use crate::config;

pub const LATEST: &str = "https://api.github.com/repos/M4S4T0-V01D/void-lodestone/releases/latest";
pub const RELEASES: &str = "https://github.com/M4S4T0-V01D/void-lodestone/releases";
const BUNDLE: &str = "void-lodestone.flatpak";

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// Without the leading "v".
    pub version: String,
    pub notes: String,
    pub url: String,
    pub published_at: String,
    /// Newer than the running version.
    pub newer: bool,
    /// The release has a Flatpak bundle and this copy can install it.
    pub installable: bool,
    #[serde(skip)]
    bundle_url: String,
    #[serde(skip)]
    digest: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// downloading | installing | done | error
    pub phase: &'static str,
    pub received: u64,
    pub total: u64,
    pub message: String,
}

/// "v1.2.3" or "1.2.3" as numbers; anything after a '-' or '+' is ignored.
pub fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = it.next()??;
    let minor = it.next().unwrap_or(Some(0))?;
    let patch = it.next().unwrap_or(Some(0))?;
    Some((major, minor, patch))
}

pub fn is_newer(tag: &str, current: &str) -> bool {
    matches!((parse(tag), parse(current)), (Some(a), Some(b)) if a > b)
}

/// Whether the host's flatpak can be reached from inside the sandbox.
pub fn can_install() -> bool {
    config::in_flatpak()
        && std::fs::read_to_string("/.flatpak-info").is_ok_and(|info| {
            info.lines().any(|l| l.starts_with("org.freedesktop.Flatpak=talk") || l.starts_with("org.freedesktop.Flatpak=own"))
        })
}

/// `--user` or `--system`, whichever installation this copy runs from.
fn scope() -> &'static str {
    let info = std::fs::read_to_string("/.flatpak-info").unwrap_or_default();
    let app_path = info.lines().find_map(|l| l.strip_prefix("app-path=")).unwrap_or("");
    if app_path.starts_with("/var/lib/flatpak") || app_path.starts_with("/usr/") {
        "--system"
    } else {
        "--user"
    }
}

pub async fn latest(http: &reqwest::Client) -> Result<Release, String> {
    fetch(http, LATEST).await
}

/// The notes of one version's release, for "What's new".
pub async fn of_version(http: &reqwest::Client, version: &str) -> Result<Release, String> {
    fetch(http, &format!("https://api.github.com/repos/M4S4T0-V01D/void-lodestone/releases/tags/v{version}")).await
}

async fn fetch(http: &reqwest::Client, url: &str) -> Result<Release, String> {
    #[derive(Deserialize)]
    struct Gh {
        tag_name: String,
        #[serde(default)]
        body: String,
        html_url: String,
        #[serde(default)]
        published_at: String,
        assets: Vec<Asset>,
    }
    #[derive(Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
        #[serde(default)]
        digest: Option<String>,
    }
    let gh: Gh = http
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("couldn't check for Lodestone updates: {e}"))?
        .json()
        .await
        .map_err(|e| format!("unexpected reply from GitHub: {e}"))?;
    let bundle = gh.assets.iter().find(|a| a.name == BUNDLE);
    Ok(Release {
        version: gh.tag_name.trim_start_matches('v').to_string(),
        newer: is_newer(&gh.tag_name, env!("CARGO_PKG_VERSION")),
        installable: bundle.is_some() && can_install(),
        notes: gh.body,
        url: gh.html_url,
        published_at: gh.published_at,
        bundle_url: bundle.map(|a| a.browser_download_url.clone()).unwrap_or_default(),
        digest: bundle.and_then(|a| a.digest.clone()).unwrap_or_default(),
    })
}

/// The bundle goes to the app's cache folder, which the host sees as ~/.var/app/<id>/cache.
fn bundle_path() -> PathBuf {
    let cache = match std::env::var_os("XDG_CACHE_HOME") {
        Some(v) if std::path::Path::new(&v).is_absolute() => PathBuf::from(v),
        _ => config::data_dir(),
    };
    cache.join("void-lodestone-update.flatpak")
}

/// Download the release's bundle, check it and install it on the host. The new version runs from the next start.
pub async fn install(http: &reqwest::Client, rel: &Release, emit: &(dyn Fn(Progress) + Send + Sync)) -> Result<(), String> {
    if !can_install() {
        return Err("This copy of Lodestone can't install updates itself; get the new version from the releases page.".into());
    }
    if rel.bundle_url.is_empty() {
        return Err(format!("Release {} has no {BUNDLE}.", rel.version));
    }
    let say = |phase, received, total, message: &str| emit(Progress { phase, received, total, message: message.into() });
    let mut res = http
        .get(&rel.bundle_url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("couldn't download the update: {e}"))?;
    let total = res.content_length().unwrap_or(0);
    let path = bundle_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("part");
    let mut f = std::fs::File::create(&tmp).map_err(|e| format!("couldn't save the update: {e}"))?;
    let mut hash = Sha256::new();
    let (mut received, mut shown) = (0u64, 0u64);
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("download interrupted: {e}"))? {
        f.write_all(&chunk).map_err(|e| e.to_string())?;
        hash.update(&chunk);
        received += chunk.len() as u64;
        if received - shown >= 512 * 1024 {
            shown = received;
            say("downloading", received, total, "");
        }
    }
    f.sync_all().map_err(|e| e.to_string())?;
    drop(f);
    let sha: String = hash.finalize().iter().map(|x| format!("{x:02x}")).collect();
    if let Some(want) = rel.digest.strip_prefix("sha256:") {
        if !want.eq_ignore_ascii_case(&sha) {
            let _ = std::fs::remove_file(&tmp);
            return Err("The update didn't match its published checksum, so it was discarded.".into());
        }
    }
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    say("installing", received, received, "");

    // The app's cache folder is ~/.var/app/<id>/cache on both sides, so the host can read the path as it is.
    let file = path.clone();
    let run = move |extra: &[&str]| {
        let mut cmd = std::process::Command::new("flatpak-spawn");
        cmd.args(["--host", "flatpak", "install", scope(), "--noninteractive", "-y"]).args(extra).arg("--bundle").arg(&file);
        cmd.output()
    };
    let out = tokio::task::spawn_blocking(move || {
        let first = run(&[])?;
        // Installed from another origin (say a local build): replace it.
        if !first.status.success() && String::from_utf8_lossy(&first.stderr).contains("already installed") {
            return run(&["--reinstall"]);
        }
        Ok(first)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("couldn't run flatpak on the host: {e}"))?;
    let _ = std::fs::remove_file(&path);
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("flatpak failed").trim();
        return Err(format!("Installing the update failed: {line}"));
    }
    say("done", received, received, "");
    Ok(())
}

/// Start a fresh copy on the host once this one has gone, so the single-instance socket doesn't wake this one.
pub fn relaunch() -> Result<(), String> {
    let id = std::env::var("FLATPAK_ID").map_err(|_| "not running as a Flatpak")?;
    std::process::Command::new("flatpak-spawn")
        .args(["--host", "sh", "-c", "sleep 1.5; exec flatpak run \"$0\"", &id])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("couldn't restart Lodestone: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert_eq!(parse("v0.10.2"), Some((0, 10, 2)));
        assert_eq!(parse("1.2"), Some((1, 2, 0)));
        assert_eq!(parse("1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(parse("nightly"), None);
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("v1.0.0", "0.3.0"));
        assert!(!is_newer("v0.3.0", "0.3.0"));
        assert!(!is_newer("v0.2.1", "0.3.0"));
        assert!(!is_newer("junk", "0.3.0"));
    }
}
