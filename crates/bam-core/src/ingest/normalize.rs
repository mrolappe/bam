//! Landing → package normalization.
//!
//! The pure derivation functions here have no I/O; the database
//! orchestration that reads `landing_index_line` and writes `package` lives
//! in `store::normalize` (invariant I1: the SQL layer is confined to
//! `store::*`).

use super::charset::decode;
use super::index::{ParseError, parse_index_line};

/// One landing line, normalized. Caller supplies `landing_id` and the
/// generated `id` when inserting.
#[derive(Debug, Clone, PartialEq)]
pub struct NewPackage {
    pub dir: String,
    pub file: String,
    pub name: String,
    pub version: Option<String>,
    pub size_bytes: Option<i64>,
    pub uploaded_on: Option<String>,
    pub date_precision: &'static str,
    pub description: Option<String>,
}

/// Parses an Aminet size field (`"134K"`, `"1.2M"`, a bare integer) to bytes,
/// base 1024. `None` for anything else — a garbage field must not silently
/// become zero.
pub fn parse_size_bytes(s: &str) -> Option<i64> {
    let (num, mult) = match s.as_bytes().last()? {
        b'K' => (&s[..s.len() - 1], 1024.0),
        b'M' => (&s[..s.len() - 1], 1024.0 * 1024.0),
        _ => (s, 1.0),
    };
    let value: f64 = num.parse().ok()?;
    (value >= 0.0).then(|| (value * mult).round() as i64)
}

/// Splits a filename into `(name, version)`. The extension (text after the
/// final `.`) is dropped. A version is only recognised as a `-`-prefixed
/// suffix that starts with an ASCII digit (Aminet's convention); anything
/// else is ambiguous, so the whole stem becomes the name rather than a guess.
pub fn split_name_version(file: &str) -> (String, Option<String>) {
    let stem = match file.rfind('.') {
        Some(i) => &file[..i],
        None => file,
    };
    match stem.rfind('-') {
        Some(i) if stem.as_bytes().get(i + 1).is_some_and(u8::is_ascii_digit) => {
            (stem[..i].to_string(), Some(stem[i + 1..].to_string()))
        }
        _ => (stem.to_string(), None),
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian civil date. Howard
/// Hinnant's `days_from_civil` (public domain,
/// <http://howardhinnant.github.io/date_algorithms.html>) — correct across
/// the full range we need without pulling in a calendar dependency.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Inverse of [`days_from_civil`].
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Derives an ISO `uploaded_on` date from a landing row's `fetched_at`
/// (RFC3339) and an INDEX/RECENT line's age-in-weeks field. Aminet's age is
/// relative to when the INDEX was generated, so the result is ±1 week —
/// callers set `date_precision = "week"`.
pub fn date_from_age_weeks(fetched_at: &str, age_weeks: i64) -> Option<String> {
    let date_part = fetched_at.get(0..10)?;
    let mut parts = date_part.splitn(3, '-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    let days = days_from_civil(y, m, d) - age_weeks * 7;
    let (y, m, d) = civil_from_days(days);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// Applies the INDEX parser, charset decode, and the derivation rules above
/// to one landing row's raw bytes.
pub fn normalize_line(raw: &[u8], fetched_at: &str) -> Result<NewPackage, ParseError> {
    let record = parse_index_line(raw)?;
    let (file, _) = decode(record.file);
    let (dir, _) = decode(record.dir);
    let (size_str, _) = decode(record.size);
    let (age_str, _) = decode(record.age);
    let (description, _) = decode(record.description);

    let (name, version) = split_name_version(&file);
    let size_bytes = parse_size_bytes(size_str.trim());
    let uploaded_on = age_str
        .trim()
        .parse::<i64>()
        .ok()
        .and_then(|weeks| date_from_age_weeks(fetched_at, weeks));

    Ok(NewPackage {
        dir,
        file,
        name,
        version,
        size_bytes,
        uploaded_on,
        date_precision: "week",
        description: (!description.is_empty()).then_some(description),
    })
}
