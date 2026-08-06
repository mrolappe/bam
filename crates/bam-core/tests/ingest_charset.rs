use bam_core::ingest::charset::decode;
use encoding_rs::{UTF_8, WINDOWS_1252};

#[test]
fn iso_8859_1_sequence_with_umlaut_decodes_correctly() {
    let text = "Über die Straße gehen heute Abend viele Mädchen und Bäume stehen dort im Wald.";
    let (bytes, _, had_errors) = WINDOWS_1252.encode(text);
    assert!(!had_errors);

    let (decoded, encoding) = decode(&bytes);
    assert_eq!(decoded, text);
    assert_eq!(encoding, WINDOWS_1252);
}

#[test]
fn utf8_sequence_decodes_correctly() {
    let text = "Über die Straße gehen heute Abend viele Mädchen und Bäume stehen dort im Wald.";

    let (decoded, encoding) = decode(text.as_bytes());
    assert_eq!(decoded, text);
    assert_eq!(encoding, UTF_8);
}

#[test]
fn ambiguous_short_input_falls_back_to_iso_8859_1() {
    // A single accented byte gives chardetng nothing to build confidence
    // from (it "wants full text rather than a prefix" — a fragment this
    // short never scores higher than the generic default).
    let (decoded, encoding) = decode(&[0xE9]);
    assert_eq!(encoding, WINDOWS_1252);
    assert_eq!(decoded, "\u{e9}");
}
