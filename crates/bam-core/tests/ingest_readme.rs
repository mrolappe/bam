//! P4.5 — readme header parser.

use bam_core::ingest::charset::decode;
use bam_core::ingest::readme::parse_readme_header;
use std::path::Path;

fn fixture_text(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/readmes")
        .join(name);
    let raw = std::fs::read(path).unwrap();
    decode(&raw).0
}

/// Twenty real readmes, sampled across the ten categories `index_sample.txt`
/// covers, parsed without error with the recognised-field count pinned per
/// fixture — a parser change shows up as a diff here, not a silent pass.
#[test]
fn twenty_real_readmes_parse_with_pinned_field_counts() {
    let cases: &[(&str, usize)] = &[
        ("biz_dbase_A2KDeck.readme", 5),
        ("biz_dbase_AA_30.readme", 5),
        ("biz_dbase_AAA_MegaBook.readme", 5),
        ("biz_dbase_AB.readme", 4),
        ("biz_misc_A-Ontime.readme", 4),
        ("biz_misc_ABank_11.readme", 4),
        ("biz_misc_ABxCD_v1.0.readme", 4),
        ("comm_bbs_ArexxCompiler.readme", 4),
        ("comm_bbs_hardchecker1_8.readme", 4),
        ("comm_cnet_edpwall.readme", 4),
        ("comm_cnet_edpwhoban2.readme", 4),
        ("comm_dlg_lcp101b.readme", 3),
        ("comm_www_AWeb36b8.readme", 5),
        ("demo_aga_CDL-90half.readme", 3),
        ("demo_euro_platon42-papagago.readme", 4),
        ("demo_euro_saxtorp-allabarnen.readme", 4),
        ("dev_cross_gcc-4.2.2-ppc-aros-cygw.readme", 5),
        ("dev_cross_gcc-4.2.2-x86_64-cygwin.readme", 5),
        ("mus_edit_ptk_v2.5.1_aros_svn_635.readme", 5),
        ("mus_edit_ptk_v2.5.2_aros_svn_639.readme", 5),
    ];
    assert_eq!(cases.len(), 20);

    for (name, expected_count) in cases {
        let header = parse_readme_header(&fixture_text(name));
        assert_eq!(
            header.field_count(),
            *expected_count,
            "{name}: got {header:?}"
        );
    }
}

#[test]
fn no_header_block_yields_empty_result() {
    let text = "This readme has no header block at all.\n\nJust prose.\n";
    assert_eq!(parse_readme_header(text), Default::default());
}

#[test]
fn a_blank_first_line_yields_empty_result() {
    assert_eq!(
        parse_readme_header("\nShort: never reached\n"),
        Default::default()
    );
}

#[test]
fn a_wrapped_multi_line_value_is_captured_whole() {
    let text = "Short:        A description that runs long enough\n              to wrap onto a second physical line.\nAuthor:       Someone\n";
    let header = parse_readme_header(text);
    assert_eq!(
        header.short.as_deref(),
        Some("A description that runs long enough to wrap onto a second physical line.")
    );
    assert_eq!(header.author.as_deref(), Some("Someone"));
}

#[test]
fn an_unrecognised_field_line_does_not_extend_the_previous_field() {
    // Real shape (comm_bbs_ArexxCompiler.readme): Architecture: isn't a
    // registered field, and must not be swallowed into Type:'s value.
    let text = "Short:  x\nType:   comm/bbs\nArchitecture: m68k-amigaos\n";
    let header = parse_readme_header(text);
    assert_eq!(header.r#type.as_deref(), Some("comm/bbs"));
    assert_eq!(header.field_count(), 2);
}
