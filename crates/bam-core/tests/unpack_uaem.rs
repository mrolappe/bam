//! P5.7: `.uaem` sidecar formatting (§12.1).

use std::time::{Duration, SystemTime};

use bam_core::unpack::{ProtectionBits, format_uaem_line};

/// 2001-03-11 22:15:00.00 UTC, matching §12.1's example line exactly.
fn example_mtime() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(984348900)
}

#[test]
fn known_attribute_set_formats_byte_exact() {
    let protection = ProtectionBits {
        r: true,
        w: true,
        e: true,
        d: true,
        ..Default::default()
    };
    let line = format_uaem_line(Some(protection), Some("Some comment"), example_mtime()).unwrap();
    assert_eq!(line, "----rwed 2001-03-11 22:15:00.00 Some comment");
}

#[test]
fn absent_flags_render_as_dash_in_hsparwed_order() {
    let protection = ProtectionBits {
        h: true,
        s: true,
        ..Default::default()
    };
    let line = format_uaem_line(Some(protection), None, example_mtime()).unwrap();
    assert!(line.starts_with("hs------ "));
}

#[test]
fn fractional_second_is_hundredths() {
    let mtime = example_mtime() + Duration::from_millis(340);
    let line = format_uaem_line(None, None, mtime).unwrap();
    assert!(line.contains("22:15:00.34"));
}

#[test]
fn no_comment_omits_trailing_field() {
    let line = format_uaem_line(None, None, example_mtime()).unwrap();
    assert_eq!(line, "-------- 2001-03-11 22:15:00.00");
    assert!(!line.ends_with(' '));
}

#[test]
fn comment_with_newline_is_rejected() {
    let err = format_uaem_line(None, Some("line one\nline two"), example_mtime());
    assert!(err.is_err());
}
