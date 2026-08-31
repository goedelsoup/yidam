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

/// Parse `YYYY-MM-DD` into days since the epoch.
///
/// `None` on anything else — including the `"unknown"` that
/// [`crate::provenance::Provenance`] degrades to, and the `last spring` a corpus writes when
/// it means it does not know.
pub fn days_from_civil_str(s: &str) -> Option<i64> {
    let mut parts = s.trim().splitn(3, '-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.trim().parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
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
}
