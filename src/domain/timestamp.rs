//! Compact UTC timestamps, as backup file names carry them.

/// `unix_seconds` as `yyyymmddThhmmssZ` in UTC (`20261004T103400Z`).
#[must_use]
pub fn compact_utc(unix_seconds: i64) -> String {
    const SECONDS_PER_DAY: i64 = 86_400;
    let days = unix_seconds.div_euclid(SECONDS_PER_DAY);
    let second_of_day = unix_seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        second_of_day / 3600,
        second_of_day % 3600 / 60,
        second_of_day % 60
    )
}

/// The proleptic Gregorian date `days` after 1970-01-01, by Howard
/// Hinnant's `civil_from_days` (eras of 400 years starting on March 1st).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_seconds_as_a_compact_utc_timestamp() {
        let cases = [
            (0, "19700101T000000Z"),
            (1_709_210_096, "20240229T123456Z"),
            (951_782_400, "20000229T000000Z"),
            (1_791_072_000, "20261004T000000Z"),
            (1_791_110_040, "20261004T103400Z"),
            (1_791_158_399, "20261004T235959Z"),
            (1_791_158_400, "20261005T000000Z"),
            (1_767_225_599, "20251231T235959Z"),
            (-1, "19691231T235959Z"),
            (-86_400, "19691231T000000Z"),
            (-2_208_988_800, "19000101T000000Z"),
        ];
        for (seconds, expected) in cases {
            assert_eq!(compact_utc(seconds), expected, "{seconds}");
        }
    }
}
