//! "Build & play": build the user's RuneLite plugin projects with Gradle and copy their jars into RuneLite's
//! sideloaded-plugins folder.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::{data_dir, in_flatpak, Settings};
use crate::launch::expand_home;

/// The project folders from settings, one per line.
pub fn projects(settings: &Settings) -> Vec<PathBuf> {
    settings
        .plugin_projects
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| PathBuf::from(expand_home(l)))
        .collect()
}

pub fn task(settings: &Settings) -> String {
    match settings.build_task.trim() {
        "" => "jar".into(),
        t => t.into(),
    }
}

/// The Gradle command for one project: its wrapper if it has one, else `gradle` from PATH.
pub fn command(project: &Path, task: &str) -> Command {
    let wrapper = project.join("gradlew");
    let mut cmd = if wrapper.is_file() {
        let mut c = Command::new("sh");
        c.arg(wrapper);
        c
    } else {
        Command::new("gradle")
    };
    // A daemon makes rebuilds fast; let it go after ten idle minutes instead of three hours.
    cmd.args(["--console=plain", "-Dorg.gradle.daemon.idletimeout=600000", task])
        .current_dir(project)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if in_flatpak() {
        // The sandbox can't write ~/.gradle, and its Java is the bundled JDK.
        cmd.env("GRADLE_USER_HOME", data_dir().join("gradle"));
        if Path::new("/app/jre/bin/javac").exists() {
            cmd.env("JAVA_HOME", "/app/jre");
        }
    }
    cmd
}

/// The plugin jars a build left in `build/libs`: not the fat (`-shadow`, `-all`), sources or javadoc jars.
pub fn built_jars(project: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(project.join("build/libs")) else { return Vec::new() };
    let mut jars: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            name.ends_with(".jar")
                && !["-shadow.jar", "-all.jar", "-sources.jar", "-javadoc.jar", "-tests.jar"].iter().any(|s| name.ends_with(s))
        })
        .collect();
    jars.sort();
    jars
}

/// Copy the jars into `dir`, replacing older copies. Returns the file names copied.
pub fn install(jars: &[PathBuf], dir: &Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    let mut names = Vec::new();
    for jar in jars {
        let name = jar.file_name().unwrap_or_default();
        // Write next to it and rename, so a running RuneLite never sees half a jar.
        let tmp = dir.join(format!(".{}.part", name.to_string_lossy()));
        std::fs::copy(jar, &tmp).map_err(|e| format!("couldn't copy {}: {e}", jar.display()))?;
        std::fs::rename(&tmp, dir.join(name)).map_err(|e| format!("couldn't copy {}: {e}", jar.display()))?;
        names.push(name.to_string_lossy().into_owned());
    }
    Ok(names)
}

/// The Java packages a project's plugins live in, from `runelite-plugin.properties` (`plugins=a.b.MyPlugin`), or
/// else its Gradle `group`. Their log lines get highlighted.
pub fn packages(project: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(props) = std::fs::read_to_string(project.join("runelite-plugin.properties")) {
        for line in props.lines() {
            if let Some(list) = line.trim().strip_prefix("plugins=") {
                for class in list.split(',').map(str::trim) {
                    if let Some((pkg, _)) = class.rsplit_once('.') {
                        out.push(pkg.to_string());
                    }
                }
            }
        }
    }
    if out.is_empty() {
        let gradle = std::fs::read_to_string(project.join("build.gradle"))
            .or_else(|_| std::fs::read_to_string(project.join("build.gradle.kts")))
            .unwrap_or_default();
        for line in gradle.lines() {
            if let Some(v) = line.trim().strip_prefix("group") {
                let v = v.trim_start_matches([' ', '=']).trim().trim_matches(['\'', '"']);
                if !v.is_empty() && v.contains('.') && v != "com.example" {
                    out.push(v.to_string());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_plugin_jars_and_packages() {
        let dir = std::env::temp_dir().join(format!("lodestone-gradle-{}", std::process::id()));
        let libs = dir.join("build/libs");
        std::fs::create_dir_all(&libs).unwrap();
        for f in ["death-recap.jar", "death-recap-shadow.jar", "death-recap-sources.jar", "notes.txt"] {
            std::fs::write(libs.join(f), b"x").unwrap();
        }
        assert_eq!(built_jars(&dir), vec![libs.join("death-recap.jar")]);

        std::fs::write(dir.join("build.gradle"), "group = 'com.deathrecap'\nversion = '2.0'\n").unwrap();
        assert_eq!(packages(&dir), vec!["com.deathrecap"]);
        std::fs::write(dir.join("runelite-plugin.properties"), "plugins=com.a.APlugin,com.b.x.BPlugin\n").unwrap();
        assert_eq!(packages(&dir), vec!["com.a", "com.b.x"]);

        let out = dir.join("sideload");
        assert_eq!(install(&built_jars(&dir), &out).unwrap(), vec!["death-recap.jar"]);
        assert!(out.join("death-recap.jar").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_project_lines() {
        let s = Settings { plugin_projects: " /a \n\n/b\n".into(), ..Settings::default() };
        assert_eq!(projects(&s), vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(task(&Settings { build_task: " ".into(), ..Settings::default() }), "jar");
    }
}
