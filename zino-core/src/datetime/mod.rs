//! ISO 8601 combined date and time with local time zone.

use chrono::{format::ParseError, Local, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
    str::FromStr,
    time::Duration,
};

mod duration;

pub use duration::{parse_duration, ParseDurationError};

/// A wrapper type for [`chrono::DateTime<Local>`](chrono::DateTime).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DateTime(chrono::DateTime<Local>);

impl DateTime {
    /// Returns a new instance which corresponds to the current date.
    #[inline]
    pub fn now() -> Self {
        Self(Local::now())
    }

    /// Returns a new instance corresponding to a UTC date and time,
    /// from the number of non-leap seconds since the midnight UTC on January 1, 1970.
    #[inline]
    pub fn from_timestamp(secs: i64) -> Self {
        let dt = NaiveDateTime::from_timestamp_opt(secs, 0).unwrap_or_default();
        Self(Local.from_utc_datetime(&dt))
    }

    /// Returns a new instance corresponding to a UTC date and time,
    /// from the number of non-leap milliseconds since the midnight UTC on January 1, 1970.
    #[inline]
    pub fn from_timestamp_millis(millis: i64) -> Self {
        let dt = NaiveDateTime::from_timestamp_millis(millis).unwrap_or_default();
        Self(Local.from_utc_datetime(&dt))
    }

    /// Returns the number of non-leap seconds since January 1, 1970 0:00:00 UTC.
    #[inline]
    pub fn timestamp(&self) -> i64 {
        self.0.timestamp()
    }

    /// Returns the number of non-leap-milliseconds since January 1, 1970 UTC.
    #[inline]
    pub fn timestamp_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }

    /// Parses an RFC 2822 date and time.
    #[inline]
    pub fn parse_utc_str(s: &str) -> Result<Self, ParseError> {
        let datetime = chrono::DateTime::parse_from_rfc2822(s)?;
        Ok(Self(datetime.with_timezone(&Local)))
    }

    /// Parses an RFC 3339 and ISO 8601 date and time.
    #[inline]
    pub fn parse_iso_str(s: &str) -> Result<Self, ParseError> {
        let datetime = chrono::DateTime::parse_from_rfc3339(s)?;
        Ok(Self(datetime.with_timezone(&Local)))
    }

    /// Returns an RFC 2822 date and time string.
    #[inline]
    pub fn to_utc_string(&self) -> String {
        let datetime = self.0.with_timezone(&Utc);
        format!("{} GMT", datetime.to_rfc2822().trim_end_matches(" +0000"))
    }

    /// Return an RFC 3339 and ISO 8601 date and time string with subseconds
    /// formatted as [`SecondsFormat::Millis`](chrono::SecondsFormat::Millis).
    #[inline]
    pub fn to_iso_string(&self) -> String {
        let datetime = self.0.with_timezone(&Utc);
        datetime.to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    /// Formats the combined date and time with the specified format string.
    /// See [`format::strftime`](chrono::format::strftime) for the supported escape sequences.
    #[inline]
    pub fn format(&self, fmt: &str) -> String {
        format!("{}", self.0.format(fmt))
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0.to_rfc3339_opts(SecondsFormat::Micros, false)
        )
    }
}

impl Default for DateTime {
    /// Returns an instance which corresponds to **the current date**.
    fn default() -> Self {
        Self::now()
    }
}

impl From<chrono::DateTime<Local>> for DateTime {
    fn from(dt: chrono::DateTime<Local>) -> Self {
        Self(dt)
    }
}

impl From<DateTime> for chrono::DateTime<Local> {
    fn from(dt: DateTime) -> Self {
        dt.0
    }
}

impl From<DateTime> for Value {
    fn from(dt: DateTime) -> Self {
        Value::String(dt.to_string())
    }
}

impl FromStr for DateTime {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        chrono::DateTime::<Local>::from_str(s).map(Self)
    }
}

impl Add<Duration> for DateTime {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Duration) -> Self {
        let duration = chrono::Duration::from_std(rhs).expect("Duration value is out of range");
        let datetime = self
            .0
            .checked_add_signed(duration)
            .expect("`DateTime + Duration` overflowed");
        Self(datetime)
    }
}

impl AddAssign<Duration> for DateTime {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

impl Sub<Duration> for DateTime {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Duration) -> Self {
        let duration = chrono::Duration::from_std(rhs).expect("Duration value is out of range");
        let datetime = self
            .0
            .checked_sub_signed(duration)
            .expect("`DateTime - Duration` overflowed");
        Self(datetime)
    }
}

impl SubAssign<Duration> for DateTime {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}
