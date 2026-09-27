//! Copy every printed line to a file so they are still there after the terminal scrolls.
//!
//! The live Geyser stream writes many times per second. A terminal only keeps a
//! short scrollback; the file keeps the whole run. Default path: `logs/arb-bot.log`.
//! Set `LOG_FILE=` (empty) to turn the file off. Set `LOG_FILE=path` to pick another file.

use std::fs::{File, OpenOptions, create_dir_all};
use std::io::{IsTerminal, Write};
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
    println!(
        "{}",
        colorize(&line, colors_enabled(std::io::stdout().is_terminal()))
    );
    append_to_file(&line);
}

pub fn emit_error(line: &str) {
    let line = with_timestamp(line);
    eprintln!(
        "{}",
        colorize(&line, colors_enabled(std::io::stderr().is_terminal()))
    );
    append_to_file(&line);
}

/// `NO_COLOR` (any value) and a non-terminal destination both turn color off.
/// The log file is always written without color codes.
fn colors_enabled(stream_is_terminal: bool) -> bool {
    stream_is_terminal && std::env::var_os("NO_COLOR").is_none()
}

/// Dim the timestamp, color the `[tag]`, and mark the words that decide what to do.
fn colorize(line: &str, enabled: bool) -> String {
    if !enabled || line.is_empty() {
        return line.to_string();
    }
    let (clock, after_clock) = split_clock(line);
    let (tag, body) = split_tag(after_clock);
    let mut colored = String::with_capacity(line.len() + 48);
    if !clock.is_empty() {
        colored.push_str(&paint(clock, "90"));
        colored.push_str("  ");
    }
    if !tag.is_empty() {
        colored.push_str(&paint(tag, tag_color(tag)));
    }
    colored.push_str(&highlight_outcomes(body));
    colored
}

/// `HH:MM:SS.mmm  ` is 14 bytes. Returns the clock and everything after the two spaces.
fn split_clock(line: &str) -> (&str, &str) {
    let bytes = line.as_bytes();
    if bytes.len() >= 14
        && bytes[2] == b':'
        && bytes[5] == b':'
        && bytes[8] == b'.'
        && bytes[12] == b' '
        && bytes[13] == b' '
        && bytes[..12]
            .iter()
            .all(|byte| byte.is_ascii_digit() || *byte == b':' || *byte == b'.')
    {
        (&line[..12], &line[14..])
    } else {
        ("", line)
    }
}

fn split_tag(line: &str) -> (&str, &str) {
    let Some(start) = line.find('[') else {
        return ("", line);
    };
    let Some(end) = line[start..].find(']') else {
        return ("", line);
    };
    let end = start + end + 1;
    (&line[start..end], &line[end..])
}

fn tag_color(tag: &str) -> &'static str {
    match tag {
        "[error]" => "1;31",
        "[best size]" => "32",
        "[spread]" => "33",
        "[pool]" => "36",
        "[decide]" => "35",
        "[simulate]" => "34",
        "[send]" => "96",
        "[trade]" => "35",
        tag if tag.starts_with("[rpc") || tag.starts_with("[geyser") => "2;36",
        _ => "37",
    }
}

/// Color the verdicts a person scans for while the stream scrolls.
fn highlight_outcomes(body: &str) -> String {
    const PHRASES: &[(&str, &str)] = &[
        ("gap beats fees → checking best size", "32"),
        ("gap smaller than fees → no arbitrage", "2"),
        ("PROFITABLE", "1;32"),
        ("CONFIRMED", "1;32"),
        ("FAILED", "1;91"),
        ("partial fill", "33"),
        ("TRADE", "1;32"),
        ("skip:", "33"),
        ("failed", "31"),
    ];
    let mut rest = body;
    let mut highlighted = String::with_capacity(body.len() + 32);
    while !rest.is_empty() {
        let earliest = PHRASES
            .iter()
            .filter_map(|(phrase, color)| rest.find(phrase).map(|index| (index, *phrase, *color)))
            .min_by_key(|(index, _, _)| *index);
        let Some((index, phrase, color)) = earliest else {
            highlighted.push_str(rest);
            break;
        };
        highlighted.push_str(&rest[..index]);
        highlighted.push_str(&paint(phrase, color));
        rest = &rest[index + phrase.len()..];
    }
    color_amount_after(&color_amount_after(&highlighted, "profit="), "net ")
}

/// Color the signed amount that follows `marker` (`+` green, `-` red), up to ` SOL`.
fn color_amount_after(line: &str, marker: &str) -> String {
    let Some(start) = line.find(marker) else {
        return line.to_string();
    };
    let amount_start = start + marker.len();
    let Some(amount_len) = line[amount_start..].find(" SOL") else {
        return line.to_string();
    };
    let amount = &line[amount_start..amount_start + amount_len];
    let color = if amount.starts_with('-') { "31" } else { "32" };
    format!(
        "{}{}{}",
        &line[..amount_start],
        paint(amount, color),
        &line[amount_start + amount_len..]
    )
}

fn paint(text: &str, color: &str) -> String {
    format!("\x1b[{color}m{text}\x1b[0m")
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

#[cfg(test)]
mod tests {
    use super::colorize;

    #[test]
    fn color_marks_the_tag_and_the_profit_and_stays_off_when_disabled() {
        let line = "12:00:00.000  [best size]  raydium→orca  profit=+0.010000 SOL  PROFITABLE";
        let colored = colorize(line, true);
        assert!(colored.contains("\x1b[32m[best size]\x1b[0m"));
        assert!(colored.contains("\x1b[32m+0.010000\x1b[0m"));
        assert!(colored.contains("\x1b[1;32mPROFITABLE\x1b[0m"));
        assert_eq!(colorize(line, false), line);
    }

    #[test]
    fn a_loss_and_a_closed_gap_are_dim_or_red() {
        let line = "12:00:00.000  [spread]  gap smaller than fees → no arbitrage";
        let colored = colorize(line, true);
        assert!(colored.contains("\x1b[33m[spread]\x1b[0m"));
        assert!(colored.contains("\x1b[2mgap smaller than fees → no arbitrage\x1b[0m"));

        let loss = "12:00:00.000  [best size]  profit=-0.002000 SOL";
        assert!(colorize(loss, true).contains("\x1b[31m-0.002000\x1b[0m"));
    }
}
