//! Civil-date arithmetic, in one place.
//!
//! Hinnant's algorithms, and no date crate for what is twenty lines of exact integer
//! arithmetic. `cmd::status` converts a Unix timestamp to a civil date; `doctor` needs the
//! inverse, to turn `.yidam.toml`'s `committed = "YYYY-MM-DD"` into a day count it can
//! subtract; and `lint::ttl` needs it to age a catalog record.
//!
//! Those were three implementations before this module existed, which is the kind that is
//! wrong in one copy and right in the other — and two of them had already written the
//! comment saying so.
//!
//! It grew back to three anyway: `cmd::export::unix_to_iso` and `cmd::status::unix_to_date_str`
//! had each inlined Hinnant again, in two different arrangements. Both now call
//! [`civil_from_days`]. If you need a civil date from a timestamp, the day count is
//! `secs / 86400` and the conversion is here.

/// Days since 1970-01-01 for a civil date. Hinnant's `days_from_civil`.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Whether `y` is a leap year in the proleptic Gregorian calendar.
pub fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// The number of days in month `m` (1-12) of year `y`, or `None` for a month that is not one.
pub fn days_in_month(y: i64, m: u32) -> Option<u32> {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(y) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

/// Parse `YYYY-MM-DD` into days since the epoch.
///
/// Exactly that shape: widths 4-2-2, ASCII digits only (no sign, no short `2024-2-3`), a month
/// of 1-12, and a day the month has — so `2024-02-31` is `None`, not 2 March. Surrounding
/// whitespace is trimmed, because callers hand it a line of `git log` output.
///
/// `None` on anything else — including the `"unknown"` that
/// [`crate::provenance::Provenance`] degrades to, and the `last spring` a corpus writes when
/// it means it does not know.
pub fn days_from_civil_str(s: &str) -> Option<i64> {
    let mut parts = s.trim().split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || [(y, 4), (m, 2), (d, 2)]
            .iter()
            .any(|(p, w)| !is_digits(p, *w))
    {
        return None;
    }
    let y: i64 = y.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    let d: u32 = d.parse().ok()?;
    if !(1..=days_in_month(y, m)?).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// `p` is exactly `width` ASCII digits.
fn is_digits(p: &str, width: usize) -> bool {
    p.len() == width && p.bytes().all(|b| b.is_ascii_digit())
}

/// Today, as days since the epoch.
pub fn today_days() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86400) as i64)
        .unwrap_or(0)
}

/// The civil date for a day count. Hinnant's `civil_from_days`, and the exact inverse of
/// [`days_from_civil`].
///
/// Added for SigV4, which needs `YYYYMMDD` for its credential scope and `YYYYMMDDTHHMMSSZ`
/// for `x-amz-date` — a signature is rejected outright if the two disagree or if either
/// drifts more than fifteen minutes from the server's clock, so this is arithmetic that has
/// to be right rather than approximately right.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A Unix timestamp as `YYYYMMDDTHHMMSSZ` — the shape `x-amz-date` takes.
///
/// UTC, unconditionally. A signer that used local time would produce signatures a server
/// rejects everywhere except one timezone, and would pass every test run in that timezone.
pub fn amz_datetime(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86400) as i64;
    let secs = unix_seconds % 86400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

#[cfg(test)]
mod civil_tests {
    use super::*;

    /// The two directions are each other's inverse, checked across four centuries — leap
    /// years, century non-leaps, and the 400-year rule all fall inside this range. A
    /// round-trip is the whole correctness argument for a pair of algorithms like these,
    /// because either one alone can only be checked against a table somebody typed.
    #[test]
    fn the_two_directions_are_inverse_across_four_centuries() {
        for day in (days_from_civil(1800, 1, 1)..days_from_civil(2200, 1, 1)).step_by(7) {
            let (y, m, d) = civil_from_days(day);
            assert_eq!(
                days_from_civil(y, m, d),
                day,
                "round trip failed at {y:04}-{m:02}-{d:02}"
            );
        }
    }

    /// …and pinned against dates a reader can check by eye, so the round trip cannot be
    /// satisfied by two functions that are wrong in the same way.
    #[test]
    fn known_dates_land_where_they_should() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        // 2000 is a leap year; 1900 was not.
        assert_eq!(civil_from_days(days_from_civil(2000, 2, 29)), (2000, 2, 29));
        assert_eq!(civil_from_days(days_from_civil(1900, 3, 1)), (1900, 3, 1));
    }

    #[test]
    fn amz_datetime_is_utc_and_zero_padded() {
        // The timestamp from AWS's own SigV4 test suite, so the format is pinned against
        // the specification rather than against this function's own output.
        let t = (days_from_civil(2015, 8, 30) as u64) * 86400 + 12 * 3600 + 36 * 60;
        assert_eq!(amz_datetime(t), "20150830T123600Z");
        assert_eq!(amz_datetime(0), "19700101T000000Z");
    }

    #[test]
    fn a_date_that_exists_parses_and_round_trips() {
        for (s, ymd) in [
            ("1970-01-01", (1970, 1, 1)),
            ("2024-02-29", (2024, 2, 29)),
            ("2000-02-29", (2000, 2, 29)),
            ("2024-04-30", (2024, 4, 30)),
            ("2024-12-31", (2024, 12, 31)),
            ("2023-02-28", (2023, 2, 28)),
            (" 2024-03-02\n", (2024, 3, 2)),
        ] {
            let days = days_from_civil_str(s).unwrap_or_else(|| panic!("{s:?} should parse"));
            assert_eq!(civil_from_days(days), ymd, "{s:?}");
        }
    }

    /// The five inputs of #1617, each of which used to read as a later day.
    #[test]
    fn the_inputs_that_were_read_as_a_later_day_are_refused() {
        for s in [
            "2024-02-31",
            "2023-02-29",
            "2024-04-31",
            "2024-2-3",
            "+2024-+3-+2",
        ] {
            assert_eq!(days_from_civil_str(s), None, "{s:?}");
        }
    }

    #[test]
    fn february_follows_the_leap_rule_including_the_centuries() {
        assert!(days_from_civil_str("2024-02-29").is_some());
        assert!(days_from_civil_str("2023-02-29").is_none());
        assert!(days_from_civil_str("2000-02-29").is_some());
        assert!(days_from_civil_str("1900-02-29").is_none());
        assert!(days_from_civil_str("2100-02-29").is_none());
        assert!(days_from_civil_str("1900-02-28").is_some());
    }

    #[test]
    fn day_31_exists_only_in_a_31_day_month() {
        for m in [4, 6, 9, 11] {
            assert_eq!(
                days_from_civil_str(&format!("2024-{m:02}-31")),
                None,
                "month {m}"
            );
            assert!(days_from_civil_str(&format!("2024-{m:02}-30")).is_some());
        }
        for m in [1, 3, 5, 7, 8, 10, 12] {
            assert!(
                days_from_civil_str(&format!("2024-{m:02}-31")).is_some(),
                "month {m}"
            );
        }
    }

    #[test]
    fn month_and_day_zero_and_out_of_range_are_refused() {
        for s in [
            "2024-00-10",
            "2024-13-01",
            "2024-01-00",
            "2024-01-32",
            "2024-12-99",
        ] {
            assert_eq!(days_from_civil_str(s), None, "{s:?}");
        }
    }

    #[test]
    fn only_four_two_two_ascii_digits_parse() {
        for s in [
            "",
            "unknown",
            "2024",
            "2024-02",
            "2024-02-",
            "24-02-03",
            "02024-02-03",
            "2024-02-03-04",
            "2024/02/03",
            "2024-02-3",
            "-2024-02-03",
            "2024-+2-03",
            "2024-02-+3",
            "２０２４-02-03",
            "2024-02- 3",
            "2024 -02-03",
        ] {
            assert_eq!(days_from_civil_str(s), None, "{s:?}");
        }
    }
}
