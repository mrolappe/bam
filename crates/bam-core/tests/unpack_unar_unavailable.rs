//! P5.4 — probing `unar` when the binary is absent. Split into its own test
//! binary because it's the one scenario that needs `PATH` genuinely broken,
//! and mutating process-global env is only safe when nothing else in the
//! same process can race it.

use std::io::Cursor;

use bam_core::blob::{BlobError, BlobHash, BlobStore};
use bam_core::unpack::{Availability, UnarUnpacker, Unpacker};

struct NullStore;
impl BlobStore for NullStore {
    fn put(&self, _bytes: impl std::io::Read) -> Result<BlobHash, BlobError> {
        unreachable!("not used in this test")
    }
    fn get(&self, _hash: &BlobHash) -> Result<impl std::io::Read, BlobError> {
        Ok(Cursor::new(Vec::<u8>::new()))
    }
    fn remove(&self, _hash: &BlobHash) -> Result<(), BlobError> {
        unreachable!("not used in this test")
    }
}

#[test]
fn unar_absent_reports_unavailable_naming_the_binary_and_install_hint() {
    let real_path = std::env::var("PATH").unwrap_or_default();
    // SAFETY: single-threaded test in this binary — nothing else reads PATH
    // concurrently.
    unsafe {
        std::env::set_var("PATH", "");
    }
    let unpacker = UnarUnpacker::new(NullStore);
    let availability = unpacker.probe();
    unsafe {
        std::env::set_var("PATH", real_path);
    }

    match availability {
        Availability::Unavailable { reason } => {
            assert!(reason.contains("unar"), "reason should name unar: {reason}");
            assert!(
                reason.to_lowercase().contains("install"),
                "reason should hint at installing: {reason}"
            );
        }
        Availability::Available => panic!("expected Unavailable with empty PATH"),
    }
}
