//! `.uaem` sidecar writer (P5.7, §12.1): FS-UAE/WinUAE read a `Foo.uaem`
//! file next to `Foo` for the Amiga attributes a host filesystem can't
//! store — protection bits and comment. Format:
//!
//! ```text
//! ----rwed 2001-03-11 22:15:00.00 Some comment
//! ```
//!
//! Pure formatting stays ungated per I1; only the actual file write is
//! `native`-gated.

use std::time::SystemTime;

use thiserror::Error;

use crate::ingest::normalize::civil_from_days;
use crate::unpack::ProtectionBits;

#[derive(Debug, Error)]
pub enum UaemError {
    #[error("uaem comment must not contain newlines")]
    InvalidComment,
    #[error("io error writing .uaem sidecar: {0}")]
    Io(#[from] std::io::Error),
}

/// Flag order is `hsparwed`, lowercase when set, `-` when absent — matches
/// [`ProtectionBits`]'s field order.
fn format_flags(protection: Option<ProtectionBits>) -> String {
    let p = protection.unwrap_or_default();
    [
        (p.h, 'h'),
        (p.s, 's'),
        (p.p, 'p'),
        (p.a, 'a'),
        (p.r, 'r'),
        (p.w, 'w'),
        (p.e, 'e'),
        (p.d, 'd'),
    ]
    .into_iter()
    .map(|(set, c)| if set { c } else { '-' })
    .collect()
}

/// Formats one `.uaem` line. `mtime` is the archive's own mtime — the
/// fractional-second field is hundredths, not the underlying nanoseconds.
pub fn format_uaem_line(
    protection: Option<ProtectionBits>,
    comment: Option<&str>,
    mtime: SystemTime,
) -> Result<String, UaemError> {
    if let Some(c) = comment {
        if c.contains('\n') || c.contains('\r') {
            return Err(UaemError::InvalidComment);
        }
    }

    let dur = mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs() as i64;
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let hh = secs_of_day / 3600;
    let mm = (secs_of_day % 3600) / 60;
    let ss = secs_of_day % 60;
    let hundredths = dur.subsec_millis() / 10;

    let mut line = format!(
        "{} {y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}.{hundredths:02}",
        format_flags(protection)
    );
    if let Some(c) = comment {
        line.push(' ');
        line.push_str(c);
    }
    Ok(line)
}

#[cfg(feature = "native")]
pub fn write_sidecar(
    target: &std::path::Path,
    protection: Option<ProtectionBits>,
    comment: Option<&str>,
    mtime: SystemTime,
) -> Result<(), UaemError> {
    let line = format_uaem_line(protection, comment, mtime)?;
    let mut sidecar_name = target
        .file_name()
        .expect("target must name a file")
        .to_os_string();
    sidecar_name.push(".uaem");
    std::fs::write(target.with_file_name(sidecar_name), line)?;
    Ok(())
}
