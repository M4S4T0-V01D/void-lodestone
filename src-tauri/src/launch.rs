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

pub struct Who<'a> {
    pub session_id: &'a str,
    pub character_id: &'a str,
    pub display_name: &'a str,
}

pub fn plan(client: Client, settings: &Settings, who: &Who) -> Result<Plan, String> {
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
            let mut c = vec![java(settings)];
            c.extend(split_args(&opts.jvm_args)?);
            c.extend(["-jar".to_string(), file]);
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
    fn plans_runelite_and_rs3() {
        let mut st = Settings { java_path: "/opt/java".into(), wrapper: "gamemoderun".into(), ..Settings::default() };
        st.runelite.jvm_args = "-Xmx1g".into();
        st.runelite.args = "--debug".into();
        let who = Who { session_id: "sess", character_id: "char", display_name: "Zezima" };
        let p = plan(Client::Runelite, &st, &who).unwrap();
        assert_eq!(p.argv[..3], s(&["gamemoderun", "/opt/java", "-Xmx1g"]));
        assert_eq!(p.argv[3], "-jar");
        assert!(p.argv[4].ends_with("RuneLite.jar"));
        assert_eq!(p.argv[5], "--debug");
        assert!(p.env.contains(&("JX_SESSION_ID".into(), "sess".into())));
        assert!(p.unset.contains(&"JX_ACCESS_TOKEN"));

        st.wrapper.clear();
        let p = plan(Client::Rs3, &st, &who).unwrap();
        assert!(p.argv[0].ends_with("/runescape"));
        assert_eq!(p.argv[1..], s(&["--configURI", crate::config::RS3_CONFIG_URI]));
        assert!(p.env.iter().any(|(k, v)| k == "SDL_VIDEODRIVER" && v == "x11"));
        assert!(p.unset.contains(&"XMODIFIERS"));
    }
}
