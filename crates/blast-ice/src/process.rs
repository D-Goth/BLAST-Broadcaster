use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

// [ICE-36]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Install {
    pub dir: PathBuf,
    pub exe: PathBuf,
    pub config: Option<PathBuf>,
    pub web: Option<PathBuf>,
    pub admin: Option<PathBuf>,
}

fn first_file(list: &[PathBuf]) -> Option<PathBuf> {
    list.iter().find(|p| p.is_file()).cloned()
}

fn first_dir(list: &[PathBuf]) -> Option<PathBuf> {
    list.iter().find(|p| p.is_dir()).cloned()
}

// [ICE-37]
pub fn find_in(dir: &Path) -> Option<Install> {
    let names = ["icecast.exe", "icecast2.exe", "icecast", "icecast2"];
    let exes: Vec<PathBuf> = names
        .iter()
        .flat_map(|n| [dir.join("bin").join(n), dir.join(n)])
        .collect();
    let exe = first_file(&exes)?;
    Some(Install {
        dir: dir.to_path_buf(),
        exe,
        config: first_file(&[
            dir.join("icecast.xml"),
            dir.join("etc").join("icecast.xml"),
            dir.join("bin").join("icecast.xml"),
        ]),
        web: first_dir(&[
            dir.join("web"),
            dir.join("share").join("icecast").join("web"),
        ]),
        admin: first_dir(&[
            dir.join("admin"),
            dir.join("share").join("icecast").join("admin"),
        ]),
    })
}

// [ICE-38]
pub fn detect() -> Vec<Install> {
    let mut found = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            let Some(root) = std::env::var_os(var).map(PathBuf::from) else {
                continue;
            };
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.contains("icecast") {
                    if let Some(i) = find_in(&e.path()) {
                        found.push(i);
                    }
                }
            }
        }
    } else {
        let layouts = [
            (
                "/usr/bin/icecast2",
                "/etc/icecast2/icecast.xml",
                "/usr/share/icecast2",
            ),
            ("/usr/bin/icecast", "/etc/icecast.xml", "/usr/share/icecast"),
            (
                "/usr/local/bin/icecast",
                "/usr/local/etc/icecast.xml",
                "/usr/local/share/icecast",
            ),
            (
                "/opt/homebrew/bin/icecast",
                "/opt/homebrew/etc/icecast.xml",
                "/opt/homebrew/share/icecast",
            ),
        ];
        for (exe, cfg, share) in layouts {
            let exe = PathBuf::from(exe);
            if !exe.is_file() {
                continue;
            }
            let share = PathBuf::from(share);
            found.push(Install {
                dir: exe
                    .parent()
                    .and_then(Path::parent)
                    .map(PathBuf::from)
                    .unwrap_or_default(),
                exe,
                config: Some(PathBuf::from(cfg)).filter(|p| p.is_file()),
                web: Some(share.join("web")).filter(|p| p.is_dir()),
                admin: Some(share.join("admin")).filter(|p| p.is_dir()),
            });
        }
    }
    found.dedup_by(|a, b| a.exe == b.exe);
    found
}

// [ICE-39]
pub struct Process {
    child: Option<Child>,
    pub pid: u32,
}

fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.stdin(Stdio::null())
}

impl Process {
    pub fn start(
        exe: &Path,
        config: &Path,
        workdir: &Path,
        console: &Path,
    ) -> Result<Self, String> {
        if let Some(d) = console.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let log = std::fs::File::create(console).map_err(|e| e.to_string())?;
        let err = log.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(exe);
        quiet(&mut cmd)
            .arg("-c")
            .arg(config)
            .current_dir(workdir)
            .stdout(log)
            .stderr(err);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("lancement de {} impossible : {e}", exe.display()))?;
        let until = Instant::now() + Duration::from_millis(1200);
        while Instant::now() < until {
            if let Ok(Some(status)) = child.try_wait() {
                let out = tail(console, 12).unwrap_or_default();
                return Err(format!(
                    "Icecast s'est arrêté aussitôt ({status}). {}",
                    if out.trim().is_empty() {
                        "Consulte son journal d'erreurs.".to_string()
                    } else {
                        out.trim().to_string()
                    }
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(Self {
            pid: child.id(),
            child: Some(child),
        })
    }

    pub fn adopt(pid: u32) -> Option<Self> {
        pid_is_icecast(pid).then_some(Self { child: None, pid })
    }

    pub fn owned(&self) -> bool {
        self.child.is_some()
    }

    pub fn alive(&mut self) -> bool {
        match &mut self.child {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => pid_is_icecast(self.pid),
        }
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if !cfg!(windows) {
            let _ = quiet(&mut Command::new("kill"))
                .args(["-TERM", &self.pid.to_string()])
                .status();
            let until = Instant::now() + Duration::from_secs(4);
            while Instant::now() < until {
                if !self.alive() {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        match &mut self.child {
            Some(c) => {
                let _ = c.kill();
                c.wait().map(|_| ()).map_err(|e| e.to_string())
            }
            None => stop_pid(self.pid),
        }
    }
}

// [ICE-40]
pub fn stop_pid(pid: u32) -> Result<(), String> {
    let status = if cfg!(windows) {
        quiet(&mut Command::new("taskkill"))
            .args(["/PID", &pid.to_string(), "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else {
        quiet(&mut Command::new("kill"))
            .args(["-KILL", &pid.to_string()])
            .status()
    };
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("arrêt du processus {pid} refusé ({s})")),
        Err(e) => Err(e.to_string()),
    }
}

// [ICE-41]
pub fn pid_is_icecast(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let name = if cfg!(target_os = "linux") {
        std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default()
    } else if cfg!(windows) {
        quiet(&mut Command::new("tasklist"))
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    } else {
        quiet(&mut Command::new("ps"))
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    name.to_lowercase().contains("icecast")
}

// [ICE-42]
pub fn tail(path: &Path, lines: usize) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let window = 256 * 1024;
    let start = len.saturating_sub(window);
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    let all: Vec<&str> = text.lines().collect();
    let skip = usize::from(start > 0);
    let from = all.len().saturating_sub(lines).max(skip.min(all.len()));
    Ok(all[from..].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_windows_style_install() {
        let dir = std::env::temp_dir().join(format!("ice-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("web")).unwrap();
        std::fs::write(dir.join("bin").join("icecast.exe"), b"").unwrap();
        std::fs::write(dir.join("icecast.xml"), b"<icecast/>").unwrap();
        let i = find_in(&dir).unwrap();
        assert_eq!(i.exe, dir.join("bin").join("icecast.exe"));
        assert_eq!(i.config, Some(dir.join("icecast.xml")));
        assert_eq!(i.web, Some(dir.join("web")));
        assert_eq!(i.admin, None);
        assert!(find_in(&dir.join("web")).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tails_the_last_lines() {
        let path = std::env::temp_dir().join(format!("ice-tail-{}.log", std::process::id()));
        let text: Vec<String> = (1..=50).map(|i| format!("ligne {i}")).collect();
        std::fs::write(&path, text.join("\n")).unwrap();
        let t = tail(&path, 3).unwrap();
        assert_eq!(t, "ligne 48\nligne 49\nligne 50");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unknown_pid_is_not_icecast() {
        assert!(!pid_is_icecast(0));
        assert!(!pid_is_icecast(std::process::id()));
    }
}
