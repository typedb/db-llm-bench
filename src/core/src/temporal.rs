//! Canonical temporal string forms, shared by every DB package so the same
//! `expected` string in a question matches across DBs regardless of each
//! driver's native formatting.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

/// "2024-01-15"
pub fn canonical_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// "2024-01-15T10:30:00.000000000" — ISO with fixed nanosecond precision.
pub fn canonical_datetime(datetime: NaiveDateTime) -> String {
    datetime.format("%Y-%m-%dT%H:%M:%S%.9f").to_string()
}

/// The canonical datetime in UTC with a trailing "Z".
pub fn canonical_datetime_utc(datetime: DateTime<Utc>) -> String {
    format!("{}Z", canonical_datetime(datetime.naive_utc()))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveTime;

    use super::*;

    #[test]
    fn canonical_forms_are_pinned() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        assert_eq!(canonical_date(date), "2024-01-15");

        let datetime = NaiveDateTime::new(date, NaiveTime::from_hms_opt(10, 30, 0).unwrap());
        assert_eq!(
            canonical_datetime(datetime),
            "2024-01-15T10:30:00.000000000"
        );
        assert_eq!(
            canonical_datetime_utc(DateTime::from_naive_utc_and_offset(datetime, Utc)),
            "2024-01-15T10:30:00.000000000Z"
        );
    }
}
