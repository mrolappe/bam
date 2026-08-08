use bam_core::plugin::{ContentAnalyzerInput, ManifestError, PluginManifest, contract_schema};

const VALID: &str = r#"
name = "protracker-analyzer"
version = "0.2.0"
api_version = 1
extension_point = "content_analyzer"
claims = ["*.mod", "mod.*", "*.med"]
"#;

#[test]
fn known_api_version_loads() {
    let manifest = PluginManifest::parse(VALID).expect("valid manifest parses");
    assert_eq!(manifest.name, "protracker-analyzer");
    assert_eq!(manifest.api_version, 1);
    assert_eq!(manifest.extension_point, "content_analyzer");
}

#[test]
fn higher_major_api_version_is_rejected_naming_both_versions() {
    let src = VALID.replace("api_version = 1", "api_version = 2");
    let err = PluginManifest::parse(&src).expect_err("newer api_version must be rejected");
    match err {
        ManifestError::UnsupportedApiVersion { found, supported } => {
            assert_eq!(found, 2);
            assert_eq!(supported, 1);
        }
        other => panic!("expected UnsupportedApiVersion, got {other}"),
    }
}

#[test]
fn malformed_manifest_names_the_offending_field() {
    let src = r#"
version = "0.2.0"
api_version = 1
extension_point = "content_analyzer"
"#;
    let err = PluginManifest::parse(src).expect_err("missing `name` must be rejected");
    assert!(
        err.to_string().contains("name"),
        "error should name the missing field, got: {err}"
    );
}

#[test]
fn claims_filtering_excludes_non_matching_files() {
    let manifest = PluginManifest::parse(VALID).unwrap();
    assert!(manifest.claims_file("intro.mod"));
    assert!(!manifest.claims_file("picture.iff"));
}

#[test]
fn contract_schema_matches_the_actual_input_type() {
    let schema = contract_schema("content_analyzer").expect("content_analyzer is known");
    let required = schema["required"]
        .as_array()
        .expect("schema has a required list")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();

    let sample = ContentAnalyzerInput {
        path: "mods/foo.mod".into(),
        size: 108_234,
        bytes_b64: "".into(),
        hint: "audio".into(),
    };
    let value = serde_json::to_value(&sample).unwrap();
    let actual_fields = value.as_object().unwrap().keys().collect::<Vec<_>>();

    for field in &required {
        assert!(
            actual_fields.iter().any(|f| f.as_str() == *field),
            "schema requires `{field}` but the serialized type doesn't carry it"
        );
    }
    assert_eq!(required.len(), actual_fields.len());
}

#[test]
fn unknown_extension_point_has_no_schema() {
    assert!(contract_schema("nonexistent").is_none());
}
