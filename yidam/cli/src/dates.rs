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
