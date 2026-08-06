use bam_core::ingest::index::{ParseError, parse_index_line};

fn fixture_lines() -> Vec<Vec<u8>> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/index_sample.txt");
    let bytes = std::fs::read(path).unwrap();
    let mut lines: Vec<Vec<u8>> = bytes.split(|&b| b == b'\n').map(<[u8]>::to_vec).collect();
    if lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
    }
    lines
}

fn find_line<'a>(lines: &'a [Vec<u8>], file_prefix: &str) -> &'a [u8] {
    lines
        .iter()
        .find(|l| l.starts_with(file_prefix.as_bytes()))
        .unwrap_or_else(|| panic!("no fixture line starting with {file_prefix:?}"))
}

#[test]
fn every_line_parses_or_is_a_recognized_preamble_line() {
    for line in fixture_lines() {
        match parse_index_line(&line) {
            Ok(_) | Err(ParseError::Preamble) => {}
            Err(e) => panic!("line {line:?} failed to parse: {e:?}"),
        }
    }
}

#[test]
fn preamble_lines_are_skipped() {
    let lines = fixture_lines();
    // The 3-line `|` banner precedes the first data line.
    for line in &lines[..3] {
        assert_eq!(parse_index_line(line), Err(ParseError::Preamble));
    }
}

#[test]
fn long_filename_overflowing_the_column_still_splits_correctly() {
    let lines = fixture_lines();
    let line = find_line(&lines, "gcc-4.2.2-x86_64-cygwin.tar.bz2");
    let record = parse_index_line(line).unwrap();
    assert_eq!(record.file, b"gcc-4.2.2-x86_64-cygwin.tar.bz2");
    assert_eq!(record.dir, b"dev/cross");
    assert_eq!(record.size, b"29M");
    assert_eq!(record.age, b"916");
    assert_eq!(record.description, b"x86_64 AROS cross GCC for cygwin");
}

#[test]
fn description_with_internal_whitespace_runs_is_preserved_exactly() {
    let lines = fixture_lines();
    let line = find_line(&lines, "hardchecker1_8.lha");
    let record = parse_index_line(line).unwrap();
    assert_eq!(record.description, b"Uploadchecker for  PMBS (German only)");
}

#[test]
fn non_ascii_bytes_in_description_round_trip() {
    let lines = fixture_lines();
    let line = find_line(&lines, "Audithec.lha");
    let record = parse_index_line(line).unwrap();
    assert!(
        record.description.contains(&0xDFu8),
        "expected a non-ASCII byte in {:?}",
        record.description
    );
}

#[test]
fn zero_size_entry_parses() {
    let lines = fixture_lines();
    let line = find_line(&lines, "AlphaBase_keyfile.lha");
    let record = parse_index_line(line).unwrap();
    assert_eq!(record.size, b"0K");
}

#[test]
fn truncated_line_yields_parse_error_not_a_panic() {
    assert_eq!(parse_index_line(b""), Err(ParseError::Truncated));
    assert_eq!(
        parse_index_line(b"OnlyOneToken"),
        Err(ParseError::Truncated)
    );
    assert_eq!(
        parse_index_line(b"Foo.lha  dir/x"),
        Err(ParseError::Truncated)
    );
}
