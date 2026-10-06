//! Reader for the CUBEMAS4.10 archive format used by F1 Challenge '99-'02.
//!
//! See `specs/MAS_FORMAT.md` for the format description. This crate only
//! parses archives owned by the caller; it never writes them.

use std::fmt;
use std::io::Read;
use std::path::Path;

/// Magic header: `CUBEMAS4.10` followed by five zero bytes (16 bytes total).
const MAGIC: [u8; 16] = *b"CUBEMAS4.10\0\0\0\0\0";
/// Size of one directory entry.
const ENTRY_SIZE: usize = 256;
/// Offset of the directory within the archive.
const DIRECTORY_OFFSET: usize = 24;
/// Number of name bytes preserved in each directory entry (20..256).
const NAME_SIZE: usize = 236;

/// One directory entry in a MAS archive.
#[derive(Debug, Clone)]
pub struct MasEntry {
    pub name: String,
    pub kind: u32,
    pub offset: u32,
    pub uncompressed_size: u32,
    pub compressed_size: u32,
    pub unknown: u32,
}

/// Errors produced while opening or reading a MAS archive.
#[derive(Debug)]
pub enum MasError {
    Io(std::io::Error),
    BadMagic,
    Truncated {
        what: &'static str,
    },
    OutOfBounds {
        entry: String,
    },
    Decompress {
        entry: String,
    },
    SizeMismatch {
        entry: String,
        expected: u32,
        got: usize,
    },
}

impl fmt::Display for MasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MasError::Io(err) => write!(f, "I/O error: {err}"),
            MasError::BadMagic => write!(f, "bad magic"),
            MasError::Truncated { what } => write!(f, "truncated {what}"),
            MasError::OutOfBounds { entry } => write!(f, "entry out of bounds: {entry}"),
            MasError::Decompress { entry } => write!(f, "decompress failed: {entry}"),
            MasError::SizeMismatch {
                entry,
                expected,
                got,
            } => write!(
                f,
                "size mismatch for {entry}: expected {expected}, got {got}"
            ),
        }
    }
}

impl std::error::Error for MasError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MasError::Io(err) => Some(err),
            _ => None,
        }
    }
}

/// An opened MAS archive, owning the raw file bytes.
pub struct MasArchive {
    bytes: Vec<u8>,
    entries: Vec<MasEntry>,
    data_base: usize,
}

impl MasArchive {
    /// Open an archive from a file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MasError> {
        let bytes = std::fs::read(path).map_err(MasError::Io)?;
        Self::from_bytes(bytes)
    }

    /// Parse an archive from an in-memory buffer.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, MasError> {
        if bytes.len() < MAGIC.len() || bytes[..MAGIC.len()] != MAGIC {
            return Err(MasError::BadMagic);
        }
        if bytes.len() < 20 {
            return Err(MasError::Truncated { what: "header" });
        }

        let count = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
        let directory_size = count
            .checked_mul(ENTRY_SIZE)
            .ok_or(MasError::Truncated { what: "directory" })?;
        let data_base = DIRECTORY_OFFSET
            .checked_add(directory_size)
            .ok_or(MasError::Truncated { what: "directory" })?;
        if data_base > bytes.len() {
            return Err(MasError::Truncated { what: "directory" });
        }

        let mut entries = Vec::with_capacity(count);
        for index in 0..count {
            let base = DIRECTORY_OFFSET + index * ENTRY_SIZE;
            let field = |at: usize| -> u32 {
                let start = base + at;
                u32::from_le_bytes([
                    bytes[start],
                    bytes[start + 1],
                    bytes[start + 2],
                    bytes[start + 3],
                ])
            };
            let name = decode_name(&bytes[base + ENTRY_SIZE - NAME_SIZE..base + ENTRY_SIZE]);
            let entry = MasEntry {
                name,
                kind: field(0),
                offset: field(4),
                uncompressed_size: field(8),
                compressed_size: field(12),
                unknown: field(16),
            };

            let end = (data_base as u64) + (entry.offset as u64) + (entry.compressed_size as u64);
            if end > bytes.len() as u64 {
                return Err(MasError::OutOfBounds { entry: entry.name });
            }
            entries.push(entry);
        }

        Ok(MasArchive {
            bytes,
            entries,
            data_base,
        })
    }

    /// The parsed directory, in file order.
    pub fn entries(&self) -> &[MasEntry] {
        &self.entries
    }

    /// Find an entry by name, ASCII case-insensitive. First match wins.
    pub fn find(&self, name: &str) -> Option<&MasEntry> {
        self.entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
    }

    /// Read and decompress one entry's payload.
    pub fn read(&self, entry: &MasEntry) -> Result<Vec<u8>, MasError> {
        let start = self.data_base + entry.offset as usize;
        let end = start + entry.compressed_size as usize;
        if end > self.bytes.len() {
            return Err(MasError::OutOfBounds {
                entry: entry.name.clone(),
            });
        }
        let payload = &self.bytes[start..end];

        if entry.compressed_size == entry.uncompressed_size {
            return Ok(payload.to_vec());
        }

        let limit = entry.uncompressed_size as u64 + 1;
        let mut decoder = flate2::read::ZlibDecoder::new(payload).take(limit);
        let mut out = Vec::new();
        decoder
            .read_to_end(&mut out)
            .map_err(|_| MasError::Decompress {
                entry: entry.name.clone(),
            })?;

        if out.len() != entry.uncompressed_size as usize {
            return Err(MasError::SizeMismatch {
                entry: entry.name.clone(),
                expected: entry.uncompressed_size,
                got: out.len(),
            });
        }
        Ok(out)
    }
}

/// Decode a NUL-terminated Latin-1 name (each byte maps to one `char`).
fn decode_name(raw: &[u8]) -> String {
    let mut name = String::new();
    for &byte in raw {
        if byte == 0 {
            break;
        }
        name.push(byte as char);
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zlib(data: &[u8]) -> Vec<u8> {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    /// Build an archive from `(name, kind, payload, stored)` tuples.
    fn build(entries: &[(&str, u32, &[u8], bool)]) -> Vec<u8> {
        let count = entries.len() as u32;
        let mut directory = Vec::new();
        let mut payload = Vec::new();

        for &(name, kind, data, stored) in entries {
            let offset = payload.len() as u32;
            let bytes = if stored { data.to_vec() } else { zlib(data) };
            let compressed_size = bytes.len() as u32;
            let uncompressed_size = data.len() as u32;

            directory.extend_from_slice(&kind.to_le_bytes());
            directory.extend_from_slice(&offset.to_le_bytes());
            directory.extend_from_slice(&uncompressed_size.to_le_bytes());
            directory.extend_from_slice(&compressed_size.to_le_bytes());
            directory.extend_from_slice(&0u32.to_le_bytes());

            let mut name_raw = [0u8; NAME_SIZE];
            name_raw[..name.len()].copy_from_slice(name.as_bytes());
            directory.extend_from_slice(&name_raw);

            payload.extend_from_slice(&bytes);
        }

        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&directory);
        out.extend_from_slice(&payload);
        out
    }

    #[test]
    fn reads_stored_and_zlib_entries() {
        let stored = b"stored payload".to_vec();
        let zlib_data = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_vec();
        let archive = MasArchive::from_bytes(build(&[
            ("car.mts", 0x11, &stored, true),
            ("tex.bmp", 0x12, &zlib_data, false),
        ]))
        .unwrap();

        assert_eq!(archive.entries().len(), 2);
        let a = archive.find("car.mts").unwrap();
        assert_eq!(archive.read(a).unwrap(), stored);
        let b = archive.find("tex.bmp").unwrap();
        assert_eq!(archive.read(b).unwrap(), zlib_data);
    }

    #[test]
    fn find_is_ascii_case_insensitive() {
        let archive =
            MasArchive::from_bytes(build(&[("Engine.MTS", 0x11, b"data", true)])).unwrap();
        assert!(archive.find("engine.mts").is_some());
        assert!(archive.find("ENGINE.MTS").is_some());
        assert!(archive.find("missing.mts").is_none());
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = build(&[]);
        bytes[0] = b'X';
        assert!(matches!(
            MasArchive::from_bytes(bytes),
            Err(MasError::BadMagic)
        ));
    }

    #[test]
    fn rejects_truncated_directory() {
        let bytes = build(&[("a.mts", 0x11, b"hello", true)]);
        let truncated = bytes[..24 + ENTRY_SIZE - 1].to_vec();
        assert!(matches!(
            MasArchive::from_bytes(truncated),
            Err(MasError::Truncated { what: "directory" })
        ));
    }

    #[test]
    fn rejects_out_of_bounds_offset() {
        let mut bytes = build(&[("a.mts", 0x11, b"hello", true)]);
        bytes[24 + 4..24 + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            MasArchive::from_bytes(bytes),
            Err(MasError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn rejects_corrupt_zlib() {
        let mut bytes = build(&[("a.bmp", 0x12, b"aaaaaaaaaaaaaaaa", false)]);
        let payload_start = 24 + ENTRY_SIZE;
        bytes[payload_start] ^= 0xFF;
        let archive = MasArchive::from_bytes(bytes).unwrap();
        let entry = archive.find("a.bmp").unwrap();
        assert!(matches!(
            archive.read(entry),
            Err(MasError::Decompress { .. })
        ));
    }

    #[test]
    fn rejects_size_mismatch() {
        let mut bytes = build(&[("a.bmp", 0x12, b"aaaaaaaaaaaaaaaa", false)]);
        bytes[24 + 8..24 + 12].copy_from_slice(&999u32.to_le_bytes());
        let archive = MasArchive::from_bytes(bytes).unwrap();
        let entry = archive.find("a.bmp").unwrap();
        assert!(matches!(
            archive.read(entry),
            Err(MasError::SizeMismatch { .. })
        ));
    }

    #[test]
    fn accepts_zero_entries() {
        let archive = MasArchive::from_bytes(build(&[])).unwrap();
        assert!(archive.entries().is_empty());
    }
}
