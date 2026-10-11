//! Log line format and rotation rule. File I/O lives in `app::logging`.

use std::fmt;

/// The log file is rotated (renamed to a single backup) at this size.
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Padded so messages line up.
        f.write_str(match self {
            Self::Info => "INFO ",
            Self::Warn => "WARN ",
            Self::Error => "ERROR",
        })
    }
}

/// One log entry on one line: `<UTC time> <LEVEL> <message>`.
pub fn format_line(unix_secs: u64, level: Level, message: &str) -> String {
    let message = message.replace("\r\n", "\n").replace('\n', " | ");
    format!("{} {level} {message}", format_timestamp(unix_secs))
}

/// RFC 3339 UTC time (`1970-01-01T00:00:00Z`) without a date crate.
pub fn format_timestamp(unix_secs: u64) -> String {
    let (days, secs) = (unix_secs / 86_400, unix_secs % 86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

pub fn needs_rotation(file_len: u64) -> bool {
    file_len >= MAX_LOG_BYTES
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's
/// `civil_from_days`, restricted to non-negative days).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_of_unix_epoch() {
        assert_eq!(format_timestamp(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn timestamp_of_a_known_date() {
        // 2026-10-11 13:45:30 UTC
        assert_eq!(format_timestamp(1_791_726_330), "2026-10-11T13:45:30Z");
    }

    #[test]
    fn timestamp_handles_leap_day() {
        // 2024-02-29 00:00:00 UTC
        assert_eq!(format_timestamp(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn line_has_time_level_and_message() {
        assert_eq!(
            format_line(0, Level::Warn, "hotkey taken"),
            "1970-01-01T00:00:00Z WARN  hotkey taken"
        );
    }

    #[test]
    fn line_stays_on_one_line() {
        assert_eq!(
            format_line(0, Level::Error, "first\r\nsecond\nthird"),
            "1970-01-01T00:00:00Z ERROR first | second | third"
        );
    }

    #[test]
    fn rotates_once_the_file_reaches_the_limit() {
        assert!(!needs_rotation(MAX_LOG_BYTES - 1));
        assert!(needs_rotation(MAX_LOG_BYTES));
    }
}
