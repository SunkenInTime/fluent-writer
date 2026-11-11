//! The two date encodings the Swift app wrote.
//!
//! `JSONEncoder`'s default (`.deferredToDate`) writes a `Date` as a number of seconds since
//! 2001-01-01T00:00:00Z. That is what `state.json` and the recovery journals hold.
//! The notes store writes ISO 8601 with milliseconds instead (`2026-11-10T09:30:00.123Z`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Deserializer, Serializer};

/// Seconds between 1970-01-01 and 2001-01-01 (`NSTimeIntervalSince1970`).
const REFERENCE_DATE_OFFSET: f64 = 978_307_200.0;

pub(crate) fn unix_seconds(t: SystemTime) -> f64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -e.duration().as_secs_f64(),
    }
}

pub(crate) fn from_unix_seconds(s: f64) -> SystemTime {
    if !s.is_finite() {
        return UNIX_EPOCH;
    }
    if s >= 0.0 {
        UNIX_EPOCH
            .checked_add(Duration::from_secs_f64(s))
            .unwrap_or(UNIX_EPOCH)
    } else {
        UNIX_EPOCH
            .checked_sub(Duration::from_secs_f64(-s))
            .unwrap_or(UNIX_EPOCH)
    }
}

/// `Date` as seconds since 2001 (`JSONEncoder`'s default).
pub(crate) mod reference {
    use super::*;

    pub fn serialize<S: Serializer>(t: &SystemTime, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(unix_seconds(*t) - REFERENCE_DATE_OFFSET)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SystemTime, D::Error> {
        let v = f64::deserialize(d)?;
        Ok(from_unix_seconds(v + REFERENCE_DATE_OFFSET))
    }
}

pub(crate) fn format_iso8601(t: SystemTime) -> String {
    let dt: DateTime<Utc> = t.into();
    dt.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub(crate) fn parse_iso8601(s: &str) -> Option<SystemTime> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc).into())
}

/// `Date` as ISO 8601 with milliseconds; reads it with or without the fraction.
pub(crate) mod iso8601 {
    use super::*;

    pub fn serialize<S: Serializer>(t: &SystemTime, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format_iso8601(*t))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SystemTime, D::Error> {
        let v = String::deserialize(d)?;
        parse_iso8601(&v).ok_or_else(|| serde::de::Error::custom(format!("Bad date {v}")))
    }
}

/// `Date?` as ISO 8601, omitted when absent.
pub(crate) mod iso8601_opt {
    use super::*;

    pub fn serialize<S: Serializer>(t: &Option<SystemTime>, s: S) -> Result<S::Ok, S::Error> {
        match t {
            Some(t) => s.serialize_str(&format_iso8601(*t)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<SystemTime>, D::Error> {
        let v = Option::<String>::deserialize(d)?;
        match v {
            None => Ok(None),
            Some(v) => parse_iso8601(&v)
                .map(Some)
                .ok_or_else(|| serde::de::Error::custom(format!("Bad date {v}"))),
        }
    }
}

/// `UUID` as Foundation's `uuidString` (upper case).
pub(crate) mod uuid_upper {
    use serde::{Deserialize, Deserializer, Serializer};
    use uuid::Uuid;

    pub fn serialize<S: Serializer>(u: &Uuid, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::uuid_string(u))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
        let v = String::deserialize(d)?;
        Uuid::parse_str(&v).map_err(serde::de::Error::custom)
    }
}

/// Foundation's `UUID().uuidString`: upper-case, hyphenated.
pub(crate) fn uuid_string(u: &uuid::Uuid) -> String {
    u.hyphenated()
        .encode_upper(&mut uuid::Uuid::encode_buffer())
        .to_string()
}

pub(crate) fn new_uuid_string() -> String {
    uuid_string(&uuid::Uuid::new_v4())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_date_round_trip() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct W {
            #[serde(with = "reference")]
            d: SystemTime,
        }
        let w: W = serde_json::from_str(r#"{"d":0}"#).unwrap();
        assert_eq!(unix_seconds(w.d), REFERENCE_DATE_OFFSET);
        let w: W = serde_json::from_str(r#"{"d":784111777.5}"#).unwrap();
        assert_eq!(serde_json::to_string(&w).unwrap(), r#"{"d":784111777.5}"#);
    }

    #[test]
    fn iso_dates() {
        let t = parse_iso8601("2026-11-10T09:30:00.123Z").unwrap();
        assert_eq!(format_iso8601(t), "2026-11-10T09:30:00.123Z");
        let t = parse_iso8601("2026-11-10T09:30:00Z").unwrap();
        assert_eq!(format_iso8601(t), "2026-11-10T09:30:00.000Z");
        assert!(parse_iso8601("yesterday").is_none());
    }
}
