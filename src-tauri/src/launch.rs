//! Turning "this client, this character" into a running game: the command line, its environment, an optional
//! wrapper (gamemoderun, mangohud, ...), and a detached process in its own session so the game outlives the launcher.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::clients::Client;
use crate::config::{data_dir, in_flatpak, Settings};

pub struct Plan {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub unset: Vec<&'static str>,
    pub cwd: PathBuf,
}

/// Split a settings field into arguments, honouring single and double quotes and backslash escapes.
pub fn split_args(s: &str) -> Result<Vec<String>, String> {
    let (mut out, mut cur, mut quote, mut any) = (Vec::new(), String::new(), None::<char>, false);
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') | (None, '\\') => match chars.next() {
                Some(n) => cur.push(n),
                None => return Err("a trailing backslash has nothing to escape".into()),
            },
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(ch);
                any = true;
            }
            (None, c) if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if quote.is_some() {
        return Err(format!("unclosed quote in: {s}"));
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

/// Put the command inside a wrapper: at its `%command%` placeholder, or after it.
pub fn wrap(wrapper: Vec<String>, cmd: Vec<String>) -> Vec<String> {
    if wrapper.is_empty() {
        return cmd;
    }
    match wrapper.iter().position(|a| a == "%command%") {
        Some(i) => {
            let mut out = wrapper[..i].to_vec();
            out.extend(cmd);
            out.extend_from_slice(&wrapper[i + 1..]);
            out
        }
        None => wrapper.into_iter().chain(cmd).collect(),
    }
}

pub fn java(settings: &Settings) -> String {
    if !settings.java_path.trim().is_empty() {
        return settings.java_path.trim().to_string();
    }
    if Path::new("/app/jre/bin/java").exists() {
        return "/app/jre/bin/java".into();
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let j = PathBuf::from(home).join("bin/java");
        if j.exists() {
            return j.to_string_lossy().into();
        }
    }
    "java".into()
}

/// The HOME the games see. The Flatpak can't write the real home, so games keep their files in the app's data.
pub fn game_home(settings: &Settings) -> Option<PathBuf> {
    if !settings.game_home.trim().is_empty() {
        return Some(PathBuf::from(settings.game_home.trim()));
    }
    in_flatpak().then(|| data_dir().join("home"))
}

/// `~/x` to the user's real home, as typed in settings.
pub fn expand_home(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(rest).to_string_lossy().into(),
        None => path.to_string(),
    }
}

/// The jar a Java client runs instead of the downloaded one, when one is set and switched on.
pub fn custom_jar(client: Client, settings: &Settings) -> Option<String> {
    let opts = match client {
        Client::Runelite => &settings.runelite,
        Client::Hdos => &settings.hdos,
        Client::Rs3 => return None,
    };
    let path = opts.jar_path.trim();
    (opts.use_jar && !path.is_empty()).then(|| expand_home(path))
}

fn runelite_dir(settings: &Settings) -> PathBuf {
    game_home(settings).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default())).join(".runelite")
}

/// Where RuneLite loads sideloaded plugins from (in developer mode).
pub fn sideload_dir(settings: &Settings) -> PathBuf {
    runelite_dir(settings).join("sideloaded-plugins")
}

pub fn screenshot_dir(settings: &Settings) -> PathBuf {
    runelite_dir(settings).join("screenshots")
}

/// The type of a PNG, JPEG, WebP or GIF picture, from its first bytes.
pub fn image_type(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if head.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        Some("image/webp")
    } else if head.starts_with(b"GIF8") {
        Some("image/gif")
    } else {
        None
    }
}

/// RuneLite's own log, which has more than its console output (plugin logging included).
pub fn client_log(settings: &Settings) -> PathBuf {
    runelite_dir(settings).join("logs/client.log")
}

/// Read a growing log from `from` onwards: at most `max` bytes, starting near the end on the first read
/// (`from == u64::MAX`) and from the top again if the file was replaced or truncated. Returns the text and the
/// offset to read from next.
pub fn read_log(path: &Path, from: u64, max: u64) -> std::io::Result<(String, u64)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let start = match from {
        u64::MAX => len.saturating_sub(max),
        n if n > len => 0,
        n => n,
    };
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.take(max).read_to_end(&mut buf)?;
    let mut end = start + buf.len() as u64;
    // Don't hand out half a line (or half a UTF-8 character); it comes whole with the next read.
    if end < len || !buf.ends_with(b"\n") {
        if let Some(nl) = buf.iter().rposition(|&b| b == b'\n') {
            end = start + nl as u64 + 1;
            buf.truncate(nl + 1);
        }
    }
    Ok((String::from_utf8_lossy(&buf).into_owned(), end))
}

pub struct Who<'a> {
    pub session_id: &'a str,
    pub character_id: &'a str,
    pub display_name: &'a str,
}

/// Lets a debugger (IntelliJ's "Remote JVM Debug") attach to the game; only on this computer, and the game doesn't wait.
pub fn jdwp_arg(port: u16) -> String {
    format!("-agentlib:jdwp=transport=dt_socket,server=y,suspend=n,address=127.0.0.1:{port}")
}

/// The first port from `base` that no game of ours uses and nothing else is listening on.
pub fn free_port(base: u16, taken: &[u16]) -> Option<u16> {
    (base..base.saturating_add(50))
        .find(|p| !taken.contains(p) && std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok())
}

fn mtime_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

/// When the custom jar or any sideloaded plugin last changed, to spot a rebuild while a game runs.
pub fn jar_stamp(jar: &str, settings: &Settings) -> u64 {
    let plugins = std::fs::read_dir(sideload_dir(settings))
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jar"))
                .map(|p| mtime_ms(&p))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    mtime_ms(Path::new(jar)).max(plugins)
}

/// Pictures in RuneLite's screenshots folder (it keeps one folder per character), newest first.
pub fn screenshots(settings: &Settings, max: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut dirs = vec![(screenshot_dir(settings), 0)];
    while let Some((dir, depth)) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            let p = e.path();
            if kind.is_dir() && depth < 4 {
                dirs.push((p, depth + 1));
            } else if kind.is_file()
                && p.extension().is_some_and(|x| ["png", "jpg", "jpeg", "webp", "gif"].contains(&x.to_string_lossy().to_lowercase().as_str()))
            {
                found.push((mtime_ms(&p), p));
            }
        }
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found.into_iter().take(max).map(|(_, p)| p).collect()
}

/// The last line in a log that looks like an error or an exception, trimmed for a notice.
pub fn last_error(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .map(str::trim)
        .find(|l| {
            !l.starts_with("at ")
                && (l.contains("Exception") || l.contains(" ERROR ") || l.starts_with("Error") || l.contains("Error:"))
        })
        .map(|l| l.chars().take(200).collect())
}

/// `debug`: open a JDWP port on this one (custom-jar launches only).
pub fn plan(client: Client, settings: &Settings, who: &Who, debug: Option<u16>) -> Result<Plan, String> {
    let file = client.file().to_string_lossy().to_string();
    let mut env = vec![
        ("JX_SESSION_ID".to_string(), who.session_id.to_string()),
        ("JX_CHARACTER_ID".to_string(), who.character_id.to_string()),
        ("JX_DISPLAY_NAME".to_string(), who.display_name.to_string()),
    ];
    // Clients take tokens over the session when both are present, and RuneLite then rejects the login.
    let mut unset = vec!["JX_ACCESS_TOKEN", "JX_REFRESH_TOKEN"];
    let cmd = match client {
        Client::Runelite | Client::Hdos => {
            let opts = if client == Client::Runelite { &settings.runelite } else { &settings.hdos };
            let jar = match custom_jar(client, settings) {
                None => file,
                Some(custom) => {
                    if !Path::new(&custom).is_file() {
                        return Err(format!(
                            "The custom jar {custom} can't be found. Check the path in Settings → Clients{}.",
                            if in_flatpak() { " (the Flatpak can read files in your Documents folder)" } else { "" }
                        ));
                    }
                    custom
                }
            };
            let mut c = vec![java(settings)];
            // Java takes user.home from the password file, not HOME, so point it at the game home too.
            if let Some(h) = game_home(settings) {
                c.push(format!("-Duser.home={}", h.to_string_lossy()));
            }
            if let Some(port) = debug {
                c.push(jdwp_arg(port));
            }
            c.extend(split_args(&opts.jvm_args)?);
            c.extend(["-jar".to_string(), jar]);
            c.extend(split_args(&opts.args)?);
            c
        }
        Client::Rs3 => {
            let uri = match settings.rs3_config_uri.trim() {
                "" => crate::config::RS3_CONFIG_URI,
                u => u,
            };
            let mut c = vec![file, "--configURI".to_string(), uri.to_string()];
            c.extend(split_args(&settings.rs3.args)?);
            env.extend(
                [
                    ("PULSE_PROP_OVERRIDE", "application.name='RuneScape' application.icon_name='runescape' media.role='game'"),
                    ("SDL_VIDEO_X11_WMCLASS", "RuneScape"),
                    ("PULSE_LATENCY_MSEC", "100"),
                    ("SDL_VIDEODRIVER", "x11"),
                ]
                .map(|(k, v)| (k.to_string(), v.to_string())),
            );
            unset.push("XMODIFIERS");
            c
        }
    };
    let home = game_home(settings);
    if let Some(h) = &home {
        env.push(("HOME".into(), h.to_string_lossy().into()));
    }
    Ok(Plan {
        argv: wrap(split_args(&settings.wrapper)?, cmd),
        env,
        unset,
        cwd: home.unwrap_or_else(|| client.dir()),
    })
}

/// Shared libraries the RS3 binary needs but can't find (it wants GTK 2 and OpenSSL 1.1).
pub fn missing_libs(binary: &Path) -> Vec<String> {
    let Ok(out) = Command::new("ldd").arg(binary).stdin(Stdio::null()).stderr(Stdio::null()).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains("not found"))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect()
}

pub fn log_path(client: Client) -> PathBuf {
    let dir = data_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    // Keep the newest few logs per client.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut old: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{}-", client.id()))))
            .collect();
        old.sort();
        let keep = 9;
        if old.len() > keep {
            for p in &old[..old.len() - keep] {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    dir.join(format!("{}-{}.log", client.id(), crate::config::now()))
}

pub fn spawn(plan: &Plan, log: &Path) -> Result<Child, String> {
    use std::os::unix::process::CommandExt;
    let (program, args) = plan.argv.split_first().ok_or("nothing to run")?;
    let _ = std::fs::create_dir_all(&plan.cwd);
    let out = std::fs::File::create(log).map_err(|e| format!("couldn't create the log file: {e}"))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(&plan.cwd).stdin(Stdio::null()).stdout(out).stderr(err);
    for k in &plan.unset {
        cmd.env_remove(k);
    }
    cmd.envs(plan.env.iter().map(|(k, v)| (k, v)));
    // SAFETY: setsid is async-signal-safe; it puts the game in its own session and process group, so it survives
    // the launcher closing and can be stopped as a group.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound if program.ends_with("java") || program == "java" => {
            "Java wasn't found. Install Java 17 or newer, or set its path in Settings → Launching.".to_string()
        }
        std::io::ErrorKind::NotFound => format!("couldn't find {program}"),
        _ => format!("couldn't start {program}: {e}"),
    })
}

/// Whether anything in the process group is still running (launchers often hand over to a child and exit).
pub fn group_alive(pgid: u32) -> bool {
    // SAFETY: signal 0 only checks for existence and permission.
    unsafe { libc::kill(-(pgid as i32), 0) == 0 }
}

pub fn stop_group(pgid: u32) {
    // SAFETY: a plain signal to a process group we created.
    unsafe {
        libc::kill(-(pgid as i32), libc::SIGTERM);
    }
}

/// For a game that ignored `stop_group`.
pub fn kill_group(pgid: u32) {
    // SAFETY: as above.
    unsafe {
        libc::kill(-(pgid as i32), libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn splits_like_a_shell() {
        assert_eq!(split_args(r#"-Xmx2g  "-Dname=a b" 'c d' e\ f"#).unwrap(), s(&["-Xmx2g", "-Dname=a b", "c d", "e f"]));
        assert_eq!(split_args(r#"x """#).unwrap(), s(&["x", ""]));
        assert!(split_args("'open").is_err());
        assert!(split_args("   ").unwrap().is_empty());
    }

    #[test]
    fn wraps_at_the_placeholder_or_in_front() {
        let cmd = s(&["java", "-jar", "x.jar"]);
        assert_eq!(wrap(s(&["gamemoderun"]), cmd.clone()), s(&["gamemoderun", "java", "-jar", "x.jar"]));
        assert_eq!(
            wrap(s(&["mangohud", "%command%", "--flag"]), cmd.clone()),
            s(&["mangohud", "java", "-jar", "x.jar", "--flag"])
        );
        assert_eq!(wrap(vec![], cmd.clone()), cmd);
    }

    #[test]
    fn knows_picture_types() {
        assert_eq!(image_type(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(image_type(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_type(b"#!/bin/sh"), None);
    }

    #[test]
    fn reads_logs_in_whole_lines() {
        let p = std::env::temp_dir().join(format!("lodestone-test-{}-log", std::process::id()));
        std::fs::write(&p, "one\ntwo\nthr").unwrap();
        let (text, at) = read_log(&p, 0, 1024).unwrap();
        assert_eq!((text.as_str(), at), ("one\ntwo\n", 8));
        std::fs::write(&p, "one\ntwo\nthree\nfour\n").unwrap();
        assert_eq!(read_log(&p, at, 1024).unwrap(), ("three\nfour\n".into(), 19));
        assert_eq!(read_log(&p, u64::MAX, 5).unwrap(), ("four\n".into(), 19));
        std::fs::write(&p, "new\n").unwrap();
        assert_eq!(read_log(&p, 19, 1024).unwrap(), ("new\n".into(), 4), "restarts after truncation");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn finds_error_lines_and_ports() {
        let log = "2026 INFO ok\njava.lang.IllegalStateException: boom\n\tat a.b.C(C.java:1)\nclosing\n";
        assert_eq!(last_error(log).as_deref(), Some("java.lang.IllegalStateException: boom"));
        assert_eq!(last_error("all fine\n"), None);
        let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = held.local_addr().unwrap().port();
        assert_ne!(free_port(port, &[]), Some(port));
        assert!(free_port(port, &[]).is_some());
    }

    #[test]
    fn plans_runelite_and_rs3() {
        let mut st = Settings { java_path: "/opt/java".into(), wrapper: "gamemoderun".into(), ..Settings::default() };
        st.runelite.jvm_args = "-Xmx1g".into();
        st.runelite.args = "--debug".into();
        let who = Who { session_id: "sess", character_id: "char", display_name: "Zezima" };
        let p = plan(Client::Runelite, &st, &who, None).unwrap();
        assert_eq!(p.argv[..3], s(&["gamemoderun", "/opt/java", "-Xmx1g"]));
        assert_eq!(p.argv[3], "-jar");
        assert!(p.argv[4].ends_with("RuneLite.jar"));
        assert_eq!(p.argv[5], "--debug");
        assert!(p.env.contains(&("JX_SESSION_ID".into(), "sess".into())));
        assert!(p.unset.contains(&"JX_ACCESS_TOKEN"));

        st.game_home = "/games".into();
        st.runelite.jar_path = "/nonexistent/dev.jar".into();
        assert!(plan(Client::Runelite, &st, &who, None).unwrap().argv[5].ends_with("RuneLite.jar"), "switched off");
        st.runelite.use_jar = true;
        assert!(plan(Client::Runelite, &st, &who, None).err().unwrap().contains("/nonexistent/dev.jar"));
        let jar = std::env::temp_dir().join("lodestone-test-dev.jar");
        std::fs::write(&jar, b"").unwrap();
        st.runelite.jar_path = jar.to_string_lossy().into();
        let p = plan(Client::Runelite, &st, &who, None).unwrap();
        assert_eq!(p.argv[2..6], s(&["-Duser.home=/games", "-Xmx1g", "-jar", &st.runelite.jar_path]));
        let p = plan(Client::Runelite, &st, &who, Some(5006)).unwrap();
        assert_eq!(p.argv[3], jdwp_arg(5006));
        assert!(p.argv[3].contains("address=127.0.0.1:5006"), "debugger only on this computer");
        let _ = std::fs::remove_file(&jar);
        st.runelite.jar_path.clear();
        st.game_home.clear();

        st.wrapper.clear();
        let p = plan(Client::Rs3, &st, &who, None).unwrap();
        assert!(p.argv[0].ends_with("/runescape"));
        assert_eq!(p.argv[1..], s(&["--configURI", crate::config::RS3_CONFIG_URI]));
        assert!(p.env.iter().any(|(k, v)| k == "SDL_VIDEODRIVER" && v == "x11"));
        assert!(p.unset.contains(&"XMODIFIERS"));
    }
}
