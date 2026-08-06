//! P1.7 — exhaustive table-driven tests for the two pure functions from
//! P1.6. Expected values are transcribed from the phase doc where given;
//! see `PROGRESS.md` Round 4 for the two size cases and two version-split
//! cases the doc left without an explicit expected value, and how they were
//! derived from the doc's own stated rule instead of guessed.

use bam_core::ingest::normalize::{parse_size_bytes, split_name_version};

#[test]
fn size_suffix_parsing() {
    let cases: &[(&str, Option<i64>)] = &[
        ("0", Some(0)),
        ("1K", Some(1024)),
        ("999K", Some(999 * 1024)),
        ("1.2M", Some(1_258_291)),
        ("512", Some(512)), // no suffix
        ("garbage", None),
    ];
    for (input, expected) in cases {
        assert_eq!(parse_size_bytes(input), *expected, "input {input:?}");
    }
}

#[test]
fn name_version_splitting() {
    let cases: &[(&str, (&str, Option<&str>))] = &[
        ("Foo-1.2.lha", ("Foo", Some("1.2"))),
        ("Foo1.2.lha", ("Foo1.2", None)),
        ("Foo.lha", ("Foo", None)),
        ("Foo-2.0beta.lha", ("Foo", Some("2.0beta"))),
        ("Mod.Foo.lha", ("Mod.Foo", None)),
    ];
    for (input, (name, version)) in cases {
        let (got_name, got_version) = split_name_version(input);
        assert_eq!(got_name, *name, "input {input:?}");
        assert_eq!(got_version.as_deref(), *version, "input {input:?}");
    }
}
