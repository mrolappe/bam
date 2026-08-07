//! LHA file-header parser (P5.6, §12.1): reads Amiga protection bits
//! (`HSPARWED`) and file comments out of LHA level-0/1/2 headers so
//! extraction can later emit `.uaem` sidecars (P5.7).
//!
//! Pure byte-slice parsing, no I/O — stays ungated per I1, same shape as
//! `unpack::detect_format`.
//!
//! ponytail: the base header layout (levels 0/1/2, the level-1/2 extended
//! header chain) is taken verbatim from the documented LHa-for-UNIX format
//! and cross-checked byte-for-byte against real archives built with the
//! system `lha` tool and its own `lha v` listing. The *Amiga-specific*
//! attribute encoding is not: no authoritative spec for it turned up (the
//! real AmigaOS LhA archiver has no available source), so the OS-ID byte
//! `'A'` and the `[protection: u32 LE][comment_len: u8][comment]` layout
//! below are a best-effort placeholder, tested only against synthetic
//! bytes this codebase constructs itself. Replace with the real layout
//! once a genuine Amiga-built `.lha` fixture and an `lha -v`-equivalent
//! oracle are available (see PROGRESS.md Round 32's deviation note).
const AMIGA_OS_ID: u8 = b'A';
const AMIGA_EXT_TYPE: u8 = 0x47;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderLevel {
    Zero,
    One,
    Two,
}

/// AmigaDOS protection bits, classically displayed as `HSPARWED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProtectionBits {
    pub h: bool,
    pub s: bool,
    pub p: bool,
    pub a: bool,
    pub r: bool,
    pub w: bool,
    pub e: bool,
    pub d: bool,
}

impl ProtectionBits {
    /// AmigaDOS FIBB_* bit layout: bits 0-3 (d,e,w,r) are *inverted* —
    /// clear means permitted; bits 4-7 (a,p,s,h) are set-means-on.
    pub fn from_amiga_u32(bits: u32) -> Self {
        Self {
            d: bits & (1 << 0) == 0,
            e: bits & (1 << 1) == 0,
            w: bits & (1 << 2) == 0,
            r: bits & (1 << 3) == 0,
            a: bits & (1 << 4) != 0,
            p: bits & (1 << 5) != 0,
            s: bits & (1 << 6) != 0,
            h: bits & (1 << 7) != 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LhaFileHeader {
    pub level: HeaderLevel,
    pub filename: String,
    pub protection: Option<ProtectionBits>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LhaHeaderError {
    #[error("truncated LHA header: needed {needed} bytes, had {available}")]
    Truncated { needed: usize, available: usize },
    #[error("unsupported LHA header level {0}")]
    UnsupportedLevel(u8),
    #[error("malformed LHA extended-header chain: block size {0} too small")]
    MalformedExtension(usize),
}

fn take(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], LhaHeaderError> {
    bytes
        .get(offset..offset + len)
        .ok_or(LhaHeaderError::Truncated {
            needed: offset + len,
            available: bytes.len(),
        })
}

/// Parses one LHA file header starting at `bytes[0]`. Returns the header
/// and the total number of bytes it occupies (size+checksum byte(s) plus
/// body), so a caller can advance past it to the compressed data or the
/// next header.
pub fn parse_lha_header(bytes: &[u8]) -> Result<(LhaFileHeader, usize), LhaHeaderError> {
    let level = *take(bytes, 20, 1)?.first().unwrap();
    match level {
        0 => parse_level0(bytes),
        1 => parse_level1_or_2(bytes, HeaderLevel::One),
        2 => parse_level1_or_2(bytes, HeaderLevel::Two),
        other => Err(LhaHeaderError::UnsupportedLevel(other)),
    }
}

fn read_filename(bytes: &[u8]) -> Result<(String, usize), LhaHeaderError> {
    let namelen = take(bytes, 21, 1)?[0] as usize;
    let name = take(bytes, 22, namelen)?;
    Ok((String::from_utf8_lossy(name).into_owned(), namelen))
}

fn parse_level0(bytes: &[u8]) -> Result<(LhaFileHeader, usize), LhaHeaderError> {
    let header_size = take(bytes, 0, 1)?[0] as usize;
    let total = header_size + 2;
    take(bytes, 0, total)?; // bounds-check the whole header up front

    let (filename, namelen) = read_filename(bytes)?;
    let standard_len = 22 + namelen + 2; // fixed fields + filename + CRC
    let mut protection = None;
    let mut comment = None;

    if total > standard_len {
        let extra = take(bytes, standard_len, total - standard_len)?;
        if extra.first() == Some(&AMIGA_OS_ID) && extra.len() >= 6 {
            let bits = u32::from_le_bytes(extra[1..5].try_into().unwrap());
            protection = Some(ProtectionBits::from_amiga_u32(bits));
            let clen = extra[5] as usize;
            if let Some(text) = extra.get(6..6 + clen) {
                comment = Some(String::from_utf8_lossy(text).into_owned());
            }
        }
    }

    Ok((
        LhaFileHeader {
            level: HeaderLevel::Zero,
            filename,
            protection,
            comment,
        },
        total,
    ))
}

fn parse_level1_or_2(
    bytes: &[u8],
    level: HeaderLevel,
) -> Result<(LhaFileHeader, usize), LhaHeaderError> {
    // Level 1's fixed part is 1-byte-sized like level 0; level 2's is a
    // 2-byte total-header-size field with no separate filename in the
    // fixed part. Only the fields this parser needs (filename via chain
    // for level 2, next-header-size location) differ between them.
    let (filename, mut cursor) = match level {
        HeaderLevel::One => {
            let (name, namelen) = read_filename(bytes)?;
            (name, 22 + namelen + 2 + 1) // + CRC + OS ID
        }
        HeaderLevel::Two => (String::new(), 21 + 2 + 1), // reserved + CRC + OS ID
        HeaderLevel::Zero => unreachable!(),
    };

    let mut filename = filename;
    let mut protection = None;
    let mut comment = None;

    let mut next_size = u16::from_le_bytes(take(bytes, cursor, 2)?.try_into().unwrap()) as usize;
    cursor += 2;

    while next_size > 0 {
        if next_size < 3 {
            return Err(LhaHeaderError::MalformedExtension(next_size));
        }
        let block = take(bytes, cursor, next_size)?;
        let ext_type = block[0];
        let data = &block[1..next_size - 2];
        match ext_type {
            0x01 => filename = String::from_utf8_lossy(data).into_owned(),
            AMIGA_EXT_TYPE if data.len() >= 5 => {
                let bits = u32::from_le_bytes(data[0..4].try_into().unwrap());
                protection = Some(ProtectionBits::from_amiga_u32(bits));
                comment = Some(String::from_utf8_lossy(&data[4..]).into_owned());
            }
            _ => {}
        }
        cursor += next_size;
        next_size =
            u16::from_le_bytes(block[next_size - 2..next_size].try_into().unwrap()) as usize;
    }

    Ok((
        LhaFileHeader {
            level,
            filename,
            protection,
            comment,
        },
        cursor,
    ))
}
