//! What recognizes a file by its content: FNV-1a, and the fingerprint of a range of a file.
//! The host fingerprints its copy the same way, so the two must stay identical
//! (record/areas/logs/01-collecting.md#how-a-file-is-recognized).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const OFFSET_BASIS: u64 = 14695981039346656037;
const PRIME: u64 = 1099511628211;

/// FNV-1a, 64-bit.
pub fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(OFFSET_BASIS, |h, &b| (h ^ u64::from(b)).wrapping_mul(PRIME))
}

/// A range this long or shorter is hashed whole.
const WHOLE: u64 = 64 * 1024;
const SAMPLES: u64 = 32;
const SAMPLE_LEN: u64 = 64;

/// Identifies bytes `from..to` of a file: all of them when there are at most 64 KB, or else 32
/// samples of 64 bytes spread evenly, the last one ending at `to`. Hashing samples keeps a look at
/// a 30 MB log to 2 KB of reads, at the cost of missing a rewrite that leaves every sample alone.
pub fn fingerprint(file: &mut File, from: u64, to: u64) -> io::Result<u64> {
    let len = to.saturating_sub(from);
    if len <= WHOLE {
        let mut buf = vec![0; len as usize];
        file.seek(SeekFrom::Start(from))?;
        file.read_exact(&mut buf)?;
        return Ok(fnv(&buf));
    }
    let mut buf = vec![0; (SAMPLES * SAMPLE_LEN) as usize];
    for (i, sample) in buf.chunks_mut(SAMPLE_LEN as usize).enumerate() {
        file.seek(SeekFrom::Start(from + (len - SAMPLE_LEN) * i as u64 / (SAMPLES - 1)))?;
        file.read_exact(sample)?;
    }
    Ok(fnv(&buf))
}

/// The fingerprint of `from..to` of the file at `path`, or `None` when it's shorter than `to` or
/// can't be read.
pub fn fingerprint_of(path: &Path, from: u64, to: u64) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    if file.metadata().ok()?.len() < to {
        return None;
    }
    fingerprint(&mut file, from, to).ok()
}

/// Reads until `buf` is full or the file ends, and returns how much it read.
pub fn read_full(file: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        match file.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(got)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_matches_the_published_test_vectors() {
        assert_eq!(fnv(b""), 0xcbf29ce484222325);
        assert_eq!(fnv(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv(b"foobar"), 0x85944171f73967e8);
    }
}
