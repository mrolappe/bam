//! Parser for Aminet's `INDEX`/`RECENT` line format.

use thiserror::Error;

/// One parsed INDEX line, as borrowed byte ranges into the original `raw`
/// slice. Decoding to text is a separate step ([`super::charset::decode`]) so
/// the landing layer keeps the original bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexRecord<'a> {
    pub file: &'a [u8],
    pub dir: &'a [u8],
    pub size: &'a [u8],
    pub age: &'a [u8],
    pub description: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ParseError {
    /// A `|`-prefixed banner/preamble line, not a data line.
    #[error("preamble line")]
    Preamble,
    #[error("truncated line: fewer than 4 fields")]
    Truncated,
}

/// Parses one line of an Aminet `INDEX` or `RECENT` file.
///
/// The format is column-aligned rather than delimiter-separated: the
/// description may itself contain runs of internal whitespace, and an
/// overlong filename pushes every later column right by the overflow amount.
/// Since `file`/`dir`/`size`/`age` never legitimately contain a space, they
/// are found by scanning for the first four whitespace-delimited tokens
/// (which correctly follows any such column shift); everything after them is
/// taken verbatim, byte-for-byte, as the description.
fn next_token<'a>(raw: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
    while raw.get(*pos) == Some(&b' ') {
        *pos += 1;
    }
    let start = *pos;
    while matches!(raw.get(*pos), Some(c) if *c != b' ') {
        *pos += 1;
    }
    (*pos > start).then(|| &raw[start..*pos])
}

pub fn parse_index_line(raw: &[u8]) -> Result<IndexRecord<'_>, ParseError> {
    if raw.first() == Some(&b'|') {
        return Err(ParseError::Preamble);
    }

    let mut pos = 0;

    let file = next_token(raw, &mut pos).ok_or(ParseError::Truncated)?;
    let dir = next_token(raw, &mut pos).ok_or(ParseError::Truncated)?;
    let size = next_token(raw, &mut pos).ok_or(ParseError::Truncated)?;
    let age = next_token(raw, &mut pos).ok_or(ParseError::Truncated)?;

    while raw.get(pos) == Some(&b' ') {
        pos += 1;
    }
    let description = &raw[pos..];

    Ok(IndexRecord {
        file,
        dir,
        size,
        age,
        description,
    })
}
