//! Calendar dates for Unix milliseconds, shared by backup ids, backup
//! retention and export names. The core has no time-zone database; the
//! local offset comes from the operating system (`localtime_r`).

/// One day in milliseconds.
pub const DAY_MS: i64 = 86_400_000;

/// Which wall clock decides where one day ends and the next begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeZone {
    /// The Mac's own time zone, with its daylight-saving rules.
    Local,
    /// A fixed offset east of UTC, in minutes. For tests, and UTC (0).
    Fixed { minutes: i32 },
}

impl TimeZone {
    pub const UTC: Self = Self::Fixed { minutes: 0 };

    /// The offset east of UTC, in milliseconds, at the instant `utc_ms`.
    pub fn offset_ms(self, utc_ms: i64) -> i64 {
        match self {
            Self::Local => local_offset_seconds(utc_ms) * 1000,
            Self::Fixed { minutes } => i64::from(minutes) * 60_000,
        }
    }

    /// The calendar day (counted from 1970-01-01) on this wall clock.
    pub fn day_of(self, utc_ms: i64) -> i64 {
        (utc_ms + self.offset_ms(utc_ms)).div_euclid(DAY_MS)
    }
}

#[cfg(unix)]
fn local_offset_seconds(utc_ms: i64) -> i64 {
    // `time_t` is 64-bit on every system Tasknotes supports.
    let seconds = utc_ms.div_euclid(1000) as libc::time_t;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::zeroed();
    // SAFETY: `localtime_r` is the thread-safe variant; it writes only into
    // the `tm` we own and returns null on failure, which is checked.
    let filled = unsafe { libc::localtime_r(&seconds, tm.as_mut_ptr()) };
    if filled.is_null() {
        return 0;
    }
    // SAFETY: a non-null result means `tm` was fully written.
    unsafe { tm.assume_init() }.tm_gmtoff
}

#[cfg(not(unix))]
fn local_offset_seconds(_utc_ms: i64) -> i64 {
    0
}

/// (year, month 1–12, day 1–31) of a day counted from 1970-01-01
/// (Howard Hinnant's `civil_from_days`).
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_dates_are_right() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_752), (2026, 10, 26));
        let leap = (0..40_000)
            .find(|&d| civil_from_days(d) == (2024, 2, 29))
            .unwrap();
        assert_eq!(civil_from_days(leap + 1), (2024, 3, 1));
    }

    #[test]
    fn a_fixed_offset_moves_the_day_boundary() {
        // 2026-10-26 23:30 UTC is already 10-27 in Jakarta (UTC+7) and still
        // 10-26 in New York (UTC-4).
        let late = 20_752 * DAY_MS + 23 * 3_600_000 + 30 * 60_000;
        assert_eq!(TimeZone::UTC.day_of(late), 20_752);
        assert_eq!(TimeZone::Fixed { minutes: 7 * 60 }.day_of(late), 20_753);
        assert_eq!(TimeZone::Fixed { minutes: -4 * 60 }.day_of(late), 20_752);
    }

    #[test]
    fn the_local_offset_is_a_whole_number_of_minutes_within_a_day() {
        let offset = TimeZone::Local.offset_ms(1_793_006_795_568);
        assert_eq!(offset % 60_000, 0);
        assert!(offset.abs() < DAY_MS);
    }
}
