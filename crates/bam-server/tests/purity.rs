//! P9.2's fifth acceptance item: this crate is a thin adapter with no SQL
//! and no query logic. Same grep-based convention as `bam-core`'s
//! `tests/purity.rs` (P0.4) — `rusqlite` may only be named through
//! `bam_core`, never directly, and no raw SQL keyword appears in source.

use std::fs;
use std::path::Path;

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn server_stays_thin() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let contents = fs::read_to_string(&path).unwrap();
        if contents.contains("rusqlite") {
            violations.push(format!("{}: names rusqlite directly", path.display()));
        }
        for kw in ["SELECT ", "INSERT INTO", "UPDATE ", "DELETE FROM"] {
            if contents.contains(kw) {
                violations.push(format!("{}: contains raw SQL ({kw})", path.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "bam-server purity violated:\n{}",
        violations.join("\n")
    );
}
