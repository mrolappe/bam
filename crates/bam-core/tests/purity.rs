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
fn core_stays_pure() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let store = src.join("store");

    let mut files = Vec::new();
    rust_files(&src, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let contents = fs::read_to_string(&path).unwrap();
        let in_store = path.starts_with(&store);

        if !in_store && contents.contains("rusqlite") {
            violations.push(format!(
                "{}: names rusqlite outside src/store/",
                path.display()
            ));
        }
        if contents.contains("println!") || contents.contains("eprintln!") {
            violations.push(format!("{}: uses println!/eprintln!", path.display()));
        }
    }

    assert!(
        violations.is_empty(),
        "core purity violated:\n{}",
        violations.join("\n")
    );
}
