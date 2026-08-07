use std::path::Path;

use bam_core::blob::BlobHash;
use bam_core::unpack::{
    ArchiveFormat, Availability, ExtractedFile, UnpackError, Unpacker, UnpackerRegistry,
    detect_format,
};

struct FakeUnpacker {
    id: &'static str,
    format: ArchiveFormat,
    availability: Availability,
}

impl Unpacker for FakeUnpacker {
    fn id(&self) -> &str {
        self.id
    }

    fn handles(&self, format: ArchiveFormat) -> bool {
        format == self.format
    }

    fn probe(&self) -> Availability {
        self.availability.clone()
    }

    fn unpack(&self, _blob: &BlobHash, _dest: &Path) -> Result<Vec<ExtractedFile>, UnpackError> {
        Ok(Vec::new())
    }
}

fn available(id: &'static str, format: ArchiveFormat) -> FakeUnpacker {
    FakeUnpacker {
        id,
        format,
        availability: Availability::Available,
    }
}

#[test]
fn lha_named_file_with_lzx_magic_bytes_detects_as_lzx() {
    // A real `-lh5-` LHA header would start with size/checksum bytes then
    // `-lh5-`; this file *claims* `.lha` via filename but its bytes are the
    // LZX signature — detection must go by bytes, not the name.
    let bytes = b"LZX\0garbage-lha-shaped-name-lies";
    assert_eq!(detect_format(bytes).unwrap(), ArchiveFormat::Lzx);
}

#[test]
fn lha_magic_bytes_detect_as_lha() {
    let mut bytes = vec![0x20, 0x00];
    bytes.extend_from_slice(b"-lh5-rest of header");
    assert_eq!(detect_format(&bytes).unwrap(), ArchiveFormat::Lha);
}

#[test]
fn unknown_format_names_leading_bytes() {
    let bytes = b"\x00\x01totally not an archive";
    let err = detect_format(bytes).unwrap_err();
    match err {
        UnpackError::UnknownFormat { leading } => {
            assert_eq!(leading, bytes[..8].to_vec());
        }
        other => panic!("expected UnknownFormat, got {other:?}"),
    }
}

#[test]
fn config_override_wins_over_automatic_choice() {
    let mut reg = UnpackerRegistry::new();
    reg.register(Box::new(available("unar", ArchiveFormat::Lha)));
    reg.register(Box::new(available("second-lha", ArchiveFormat::Lha)));

    let chosen = reg.select(ArchiveFormat::Lha, Some("second-lha")).unwrap();
    assert_eq!(chosen.id(), "second-lha");
}

#[test]
fn unavailable_unpacker_is_skipped_not_attempted() {
    let mut reg = UnpackerRegistry::new();
    reg.register(Box::new(FakeUnpacker {
        id: "broken",
        format: ArchiveFormat::Lha,
        availability: Availability::Unavailable {
            reason: "binary not found".to_string(),
        },
    }));
    reg.register(Box::new(available("working", ArchiveFormat::Lha)));

    let chosen = reg.select(ArchiveFormat::Lha, None).unwrap();
    assert_eq!(chosen.id(), "working");
}

#[test]
fn no_available_unpacker_names_format_and_install_hint() {
    let mut reg = UnpackerRegistry::new();
    reg.register(Box::new(FakeUnpacker {
        id: "broken",
        format: ArchiveFormat::Lzx,
        availability: Availability::Unavailable {
            reason: "binary not found".to_string(),
        },
    }));

    let msg = match reg.select(ArchiveFormat::Lzx, None) {
        Ok(_) => panic!("expected no available unpacker"),
        Err(err) => err.to_string(),
    };
    assert!(msg.contains("LZX"), "message should name the format: {msg}");
    assert!(
        msg.to_lowercase().contains("install"),
        "message should hint at installing a backend: {msg}"
    );
}
