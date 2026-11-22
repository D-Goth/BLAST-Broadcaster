use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// [JRN-01]
const MAX_BYTES: u64 = 1024 * 1024;

// [JRN-02]
struct Journal {
    file: Mutex<File>,
    path: PathBuf,
}

static JOURNAL: OnceLock<Journal> = OnceLock::new();

// [JRN-03]
pub fn init(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let _ = JOURNAL.set(Journal {
        file: Mutex::new(file),
        path: path.to_path_buf(),
    });
    info(format!(
        "démarrage de BLAST Broadcaster {}",
        env!("CARGO_PKG_VERSION")
    ));
    Ok(())
}

// [JRN-04]
pub fn path() -> Option<PathBuf> {
    JOURNAL.get().map(|j| j.path.clone())
}

// [JRN-05]
pub fn log(level: &str, message: &str) {
    let line = format!(
        "{} [{}] {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        level.to_uppercase(),
        message
    );
    match JOURNAL.get().and_then(|j| j.file.lock().ok()) {
        Some(mut f) => {
            let _ = f.write_all(line.as_bytes());
        }
        None => eprint!("{line}"),
    }
}

// [JRN-06]
pub fn info(message: impl AsRef<str>) {
    log("info", message.as_ref());
}

// [JRN-07]
pub fn warn(message: impl AsRef<str>) {
    log("warn", message.as_ref());
}

// [JRN-08]
pub fn error(message: impl AsRef<str>) {
    log("error", message.as_ref());
}
