//! Charset detection and decoding for Aminet text (§13 of the handoff doc).
//!
//! Aminet's de-facto encoding is ISO-8859-1. `encoding_rs` models that
//! legacy label as `WINDOWS_1252` per the WHATWG Encoding Standard (the two
//! agree everywhere except the rarely-used 0x80-0x9F range), so it is the
//! fallback used here.

use chardetng::EncodingDetector;
use encoding_rs::{Encoding, WINDOWS_1252};

/// Decodes `bytes` to text, detecting the encoding with `chardetng`.
///
/// Falls back to ISO-8859-1 (`WINDOWS_1252`) when detection reports low
/// confidence, since that is Aminet's de-facto encoding. The returned label
/// should be persisted alongside the text everywhere it is stored, so a
/// later correction never requires a re-fetch.
pub fn decode(bytes: &[u8]) -> (String, &'static Encoding) {
    let mut detector = EncodingDetector::new();
    // chardetng's detection quality depends on seeing the whole text, not a
    // prefix, so the entire input is fed in one `last = true` call.
    detector.feed(bytes, true);
    let (guess, confident) = detector.guess_assess(None, true);
    let encoding = if confident { guess } else { WINDOWS_1252 };

    let (text, _, _) = encoding.decode(bytes);
    (text.into_owned(), encoding)
}
