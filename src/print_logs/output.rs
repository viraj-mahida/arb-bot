//! Copy every printed line to a file so they are still there after the terminal scrolls.
//!
//! The live Geyser stream writes many times per second. A terminal only keeps a
//! short scrollback; the file keeps the whole run. Default path: `logs/arb-bot.log`.
//! Set `LOG_FILE=` (empty) to turn the file off. Set `LOG_FILE=path` to pick another file.

use std::fs::{File, OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_LOG_FILE: &str = "logs/arb-bot.log";

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Open the log file (append) before the first line is printed.
pub fn start_copying_to_file() {
    let Some(path) = log_file_from_env() else {
        return;
    };
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        if let Err(error) = create_dir_all(parent) {
            eprintln!(
                "[error] could not create log directory {}: {error}",
                parent.display()
            );
            return;
        }
    }
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            let started_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(
                file,
                "\n======== arb-bot started unix={started_at} ========"
            );
            let _ = file.flush();
            let _ = LOG_PATH.set(path);
            let _ = LOG_FILE.set(Mutex::new(file));
        }
        Err(error) => eprintln!(
            "[error] could not open log file {}: {error}",
            path.display()
        ),
    }
}

pub fn log_file_path() -> Option<&'static Path> {
    LOG_PATH.get().map(PathBuf::as_path)
}

/// `LOG_VERBOSE=true` also prints every tick-array update.
pub fn verbose() -> bool {
    static VERBOSE: OnceLock<bool> = OnceLock::new();
    *VERBOSE.get_or_init(|| {
        matches!(
            std::env::var("LOG_VERBOSE").as_deref().map(str::trim),
            Ok("true" | "1" | "yes")
        )
    })
}

pub fn emit(line: &str) {
    let line = with_timestamp(line);
    println!("{line}");
    append_to_file(&line);
}

pub fn emit_error(line: &str) {
    let line = with_timestamp(line);
    eprintln!("{line}");
    append_to_file(&line);
}

/// Tagged lines (`[pool] …`) get a UTC `HH:MM:SS.mmm` prefix; banner text stays as is.
fn with_timestamp(line: &str) -> String {
    if !line.starts_with('[') {
        return line.to_string();
    }
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds_today = since_epoch.as_secs() % 86_400;
    format!(
        "{:02}:{:02}:{:02}.{:03}  {line}",
        seconds_today / 3600,
        seconds_today / 60 % 60,
        seconds_today % 60,
        since_epoch.subsec_millis()
    )
}

fn append_to_file(line: &str) {
    let Some(file) = LOG_FILE.get() else {
        return;
    };
    let Ok(mut file) = file.lock() else {
        return;
    };
    let _ = writeln!(file, "{line}");
    let _ = file.flush();
}

fn log_file_from_env() -> Option<PathBuf> {
    match std::env::var("LOG_FILE") {
        Err(_) => Some(PathBuf::from(DEFAULT_LOG_FILE)),
        Ok(value) if value.trim().is_empty() => None,
        Ok(value) => Some(PathBuf::from(value.trim())),
    }
}

macro_rules! log_line {
    () => {
        $crate::print_logs::output::emit("")
    };
    ($($arg:tt)*) => {
        $crate::print_logs::output::emit(&format!($($arg)*))
    };
}

macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::print_logs::output::emit_error(&format!($($arg)*))
    };
}
