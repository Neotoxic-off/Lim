//! Timestamps and dates (`<Leader>xt`): a Unix time or a date in every form, or the time between two.

use std::time::{SystemTime, UNIX_EPOCH};

use jiff::Timestamp;
use jiff::tz::TimeZone;

pub(super) fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// Seconds east of UTC in the system's time zone at unix time `ts` (0 when it is out of range).
pub(super) fn utc_offset(ts: i64) -> i64 {
    Timestamp::from_second(ts).map_or(0, |t| i64::from(TimeZone::system().to_offset(t).seconds()))
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian), after Howard Hinnant's `days_from_civil`.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y: i64 = if month <= 2 { year - 1 } else { year };
    let era: i64 = y.div_euclid(400);
    let yoe: i64 = y - era * 400;
    let doy: i64 = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe: i64 = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day count since 1970-01-01: `(year, month, day)`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z: i64 = days + 719_468;
    let era: i64 = z.div_euclid(146_097);
    let doe: i64 = z - era * 146_097;
    let yoe: i64 = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy: i64 = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp: i64 = (5 * doy + 2) / 153;
    let day: i64 = doy - (153 * mp + 2) / 5 + 1;
    let month: i64 = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

const WEEKDAYS: [&str; 7] = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"];

/// `YYYY-MM-DD HH:MM:SS` of unix time `ts` shifted by `offset` seconds.
pub(super) fn datetime(ts: i64, offset: i64) -> String {
    let local: i64 = ts + offset;
    let (days, secs): (i64, i64) = (local.div_euclid(86_400), local.rem_euclid(86_400));
    let (y, m, d): (i64, i64, i64) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

fn zone(offset: i64) -> String {
    let sign: char = if offset < 0 { '-' } else { '+' };
    format!("UTC{sign}{:02}:{:02}", offset.abs() / 3600, offset.abs() / 60 % 60)
}

/// A unix time described: local and UTC dates, weekday.
fn describe(ts: i64) -> String {
    let offset: i64 = utc_offset(ts);
    let weekday: &str = WEEKDAYS[usize::try_from(ts.div_euclid(86_400).rem_euclid(7)).unwrap_or(0)];
    format!("{ts} · {} {} · {} UTC · {weekday}", datetime(ts, offset), zone(offset), datetime(ts, 0))
}

/// `2026-10-18`, `2026-10-18 14:30`, `2026-10-18T14:30:05Z` (Z: UTC, else local time) as a unix time; the number
/// of words it used.
fn parse_date(words: &[&str]) -> Option<(i64, usize)> {
    let first: &str = words.first()?;
    let (date, mut clock): (&str, Option<&str>) = match first.split_once('T') {
        Some((date, clock)) => (date, Some(clock)),
        None => (first, None),
    };
    let mut used: usize = 1;
    if clock.is_none()
        && let Some(next) = words.get(1).copied().filter(|w| w.contains(':') && !w.contains('-'))
    {
        (clock, used) = (Some(next), 2);
    }
    let parts: Vec<i64> =
        date.trim_end_matches('Z').split('-').map(str::parse).collect::<Result<Vec<i64>, _>>().ok()?;
    let [y, m, d]: [i64; 3] = parts.try_into().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let utc: bool = clock.is_some_and(|c| c.ends_with('Z')) || date.ends_with('Z');
    let clock: Vec<i64> = match clock.map(|c| c.trim_end_matches('Z')) {
        Some(c) => c.split(':').map(str::parse).collect::<Result<Vec<i64>, _>>().ok()?,
        None => Vec::new(),
    };
    if clock.len() > 3 || clock.iter().any(|n| !(0..60).contains(n)) || clock.first().is_some_and(|h| *h > 23) {
        return None;
    }
    let secs: i64 = clock.iter().chain([&0, &0, &0]).take(3).fold(0, |acc, n| acc * 60 + n);
    let local: i64 = days_from_civil(y, m, d) * 86_400 + secs;
    // The offset in effect at that local time: taken at the UTC guess, which is right outside the hour of a switch.
    Some((if utc { local } else { local - utc_offset(local) }, used))
}

/// `now`, a unix time (seconds, or milliseconds when it is that large), a date, or two dates (their difference).
pub fn time(input: &str, now: i64) -> Result<String, String> {
    let text: &str = input.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("now") {
        return Ok(describe(now));
    }
    if let Ok(n) = text.parse::<i64>() {
        return Ok(if n.abs() > 100_000_000_000 {
            format!("{} ms · {}", n, describe(n.div_euclid(1000)))
        } else {
            describe(n)
        });
    }
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|w| !matches!(w.to_ascii_lowercase().as_str(), "to" | "-" | "à" | "->"))
        .collect();
    let (start, used): (i64, usize) = parse_date(&words).ok_or("a date is YYYY-MM-DD [HH:MM[:SS]]")?;
    if used == words.len() {
        return Ok(describe(start));
    }
    let (end, more): (i64, usize) = parse_date(&words[used..]).ok_or("a date is YYYY-MM-DD [HH:MM[:SS]]")?;
    if used + more != words.len() {
        return Err("at most two dates".into());
    }
    let secs: i64 = (end - start).abs();
    let sign: &str = if end < start { "-" } else { "" };
    Ok(format!("{sign}{} days {} h {} min · {sign}{secs} s", secs / 86_400, secs / 3600 % 24, secs / 60 % 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(2000, 2, 29), 11_016);
        for days in [-800_000, -1, 0, 59, 11_016, 20_000, 3_000_000] {
            let (y, m, d): (i64, i64, i64) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(datetime(1_700_000_000, 0), "2023-11-14 22:13:20");
        assert!(time("1700000000", 0).unwrap().contains("2023-11-14 22:13:20 UTC · Tuesday"));
        assert!(time("1700000000000", 0).unwrap().starts_with("1700000000000 ms · 1700000000"));
        assert!(time("2023-11-14T22:13:20Z", 0).unwrap().starts_with("1700000000 ·"));
        assert_eq!(time("2026-01-01Z to 2026-01-08T12:00Z", 0).unwrap(), "7 days 12 h 0 min · 648000 s");
        assert!(time("2026-13-01", 0).is_err());
        assert_eq!(time("2026-01-01Z 2026-01-02Z 2026-01-03Z", 0).unwrap_err(), "at most two dates");
        assert!(time("2026-01-01 25:00", 0).is_err());
        assert!(time("2026-01-01 1:2:3:4", 0).is_err());
        assert!(time("2026-01-01 x:00", 0).is_err());
        assert!(time("2026-01-02Z 2026-01-01Z", 0).unwrap().starts_with("-1 days"));
        assert!(time("now", 1_700_000_000).unwrap().starts_with("1700000000 ·"));
        assert!(time("", 1_700_000_000).unwrap().starts_with("1700000000 ·"));
        assert!(time("soon", 0).is_err());
    }
}
