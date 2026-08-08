//! P5.6: LHA extended-header reader.
//!
//! The three real fixtures (`lha_header_level{0,1,2}.lha`) were built with
//! the system `lha` tool and independently cross-checked byte-for-byte
//! against `lha v`'s own listing (filename, size, CRC, UID) while writing
//! the parser — see `unpack::lha_header`'s module doc. They carry no Amiga
//! attributes (the system `lha` has no Amiga awareness), so they exercise
//! base header parsing across all three levels and the "no extension
//! present" default path with a real, verifiable oracle.
//!
//! The Amiga protection-bits/comment path has no such oracle available in
//! this environment (see PROGRESS.md Round 32) and is tested against
//! synthetic bytes this test constructs itself, matching the parser's own
//! best-effort layout — explicitly *not* validated against a real Amiga
//! archive or `lha -v` yet.

use std::fs;

use bam_core::unpack::{HeaderLevel, ProtectionBits, list_headers, parse_lha_header};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(format!("tests/fixtures/archives/{name}")).unwrap()
}

#[test]
fn level0_real_fixture_parses_with_no_amiga_extension() {
    let bytes = fixture("lha_header_level0.lha");
    let (header, consumed) = parse_lha_header(&bytes).unwrap();
    assert_eq!(header.level, HeaderLevel::Zero);
    assert_eq!(header.filename, "a.txt");
    assert_eq!(header.protection, None);
    assert_eq!(header.comment, None);
    assert_eq!(&bytes[consumed..consumed + 5], b"hello");
}

#[test]
fn level1_real_fixture_parses_with_no_amiga_extension() {
    let bytes = fixture("lha_header_level1.lha");
    let (header, consumed) = parse_lha_header(&bytes).unwrap();
    assert_eq!(header.level, HeaderLevel::One);
    assert_eq!(header.filename, "a.txt");
    assert_eq!(header.protection, None);
    assert_eq!(header.comment, None);
    assert_eq!(&bytes[consumed..consumed + 5], b"hello");
}

#[test]
fn level2_real_fixture_parses_with_no_amiga_extension() {
    let bytes = fixture("lha_header_level2.lha");
    let (header, consumed) = parse_lha_header(&bytes).unwrap();
    assert_eq!(header.level, HeaderLevel::Two);
    assert_eq!(header.filename, "a.txt");
    assert_eq!(header.protection, None);
    assert_eq!(header.comment, None);
    assert_eq!(&bytes[consumed..consumed + 5], b"hello");
}

/// Builds a synthetic level-0 header carrying this parser's own best-effort
/// Amiga extension: `['A'][protection: u32 LE][comment_len: u8][comment]`
/// appended after the standard CRC.
fn synthetic_level0_with_amiga_extension(protection_bits: u32, comment: &[u8]) -> Vec<u8> {
    let filename = b"s.txt";
    let mut standard = vec![
        0u8,
        0, // header size, checksum (patched below)
        b'-',
        b'l',
        b'h',
        b'0',
        b'-', // method
        0,
        0,
        0,
        0, // compressed size
        0,
        0,
        0,
        0, // original size
        0,
        0,
        0,
        0,    // timestamp
        0x20, // attribute
        0,    // level 0
        filename.len() as u8,
    ];
    standard.extend_from_slice(filename);
    standard.extend_from_slice(&[0, 0]); // CRC (unchecked by this parser)
    standard.push(b'A');
    standard.extend_from_slice(&protection_bits.to_le_bytes());
    standard.push(comment.len() as u8);
    standard.extend_from_slice(comment);
    let header_size = standard.len() - 2;
    standard[0] = header_size as u8;
    standard
}

#[test]
fn synthetic_level0_s_bit_amiga_extension_decodes() {
    // Script bit (bit 6) set, everything else clear.
    let bytes = synthetic_level0_with_amiga_extension(0b0100_0000, b"a script");
    let (header, _) = parse_lha_header(&bytes).unwrap();
    assert_eq!(
        header.protection,
        Some(ProtectionBits {
            s: true,
            r: true,
            w: true,
            e: true,
            d: true,
            ..Default::default()
        })
    );
    assert_eq!(header.comment.as_deref(), Some("a script"));
}

#[test]
fn synthetic_level0_e_bit_amiga_extension_decodes() {
    // Executable bit clear (bit 1 = 0) means e is *set*; every other rwed
    // bit set (clear permission) except e.
    let bytes = synthetic_level0_with_amiga_extension(0b0000_1101, b"");
    let (header, _) = parse_lha_header(&bytes).unwrap();
    let p = header.protection.unwrap();
    assert!(p.e, "executable bit should decode as set");
    assert!(!p.r && !p.w && !p.d);
    assert_eq!(header.comment.as_deref(), Some(""));
}

#[test]
fn truncated_header_errors_rather_than_reading_past_buffer() {
    let bytes = fixture("lha_header_level1.lha");
    let truncated = &bytes[..25]; // cuts off mid filename/CRC
    assert!(parse_lha_header(truncated).is_err());
}

#[test]
fn truncated_amiga_extension_chain_errors_rather_than_panicking() {
    // A level-1 extended header whose declared block size is too small to
    // contain even a type byte + next-size — must error, not panic.
    let mut bytes = fixture("lha_header_level1.lha");
    // Overwrite the first extension header's next-header-size (offset
    // 30-31, confirmed against the real fixture bytes) with 1: too small
    // for a valid block.
    bytes[30] = 1;
    bytes[31] = 0;
    assert!(parse_lha_header(&bytes).is_err());
}

/// Builds one synthetic level-0 header (no Amiga extension) for `filename`
/// immediately followed by `data`, with `compressed_size` set to
/// `data.len()` — the shape `list_headers` needs to skip to the next entry.
fn synthetic_level0_entry(filename: &str, data: &[u8]) -> Vec<u8> {
    let filename = filename.as_bytes();
    let mut header = vec![
        0u8, 0, // header size, checksum (patched below)
        b'-', b'l', b'h', b'0', b'-', // method
    ];
    header.extend_from_slice(&(data.len() as u32).to_le_bytes()); // compressed size
    header.extend_from_slice(&0u32.to_le_bytes()); // original size
    header.extend_from_slice(&0u32.to_le_bytes()); // timestamp
    header.push(0x20); // attribute
    header.push(0); // level 0
    header.push(filename.len() as u8);
    header.extend_from_slice(filename);
    header.extend_from_slice(&[0, 0]); // CRC
    let header_size = header.len() - 2;
    header[0] = header_size as u8;
    header.extend_from_slice(data);
    header
}

#[test]
fn list_headers_walks_multiple_entries_by_skipping_compressed_data() {
    let mut bytes = synthetic_level0_entry("a.txt", b"hello");
    bytes.extend(synthetic_level0_entry("b.txt", b"world!"));
    bytes.push(0); // terminator

    let headers = list_headers(&bytes);
    let names: Vec<&str> = headers.iter().map(|h| h.filename.as_str()).collect();
    assert_eq!(names, vec!["a.txt", "b.txt"]);
}

#[test]
fn list_headers_on_terminator_only_bytes_returns_empty() {
    let bytes = vec![0u8, 1, 2, 3];
    assert!(list_headers(&bytes).is_empty());
}
