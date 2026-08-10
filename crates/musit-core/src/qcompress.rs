//! Qt `qCompress`/`qUncompress`-compatible framing.
//!
//! Qt prepends a 4-byte big-endian length (the uncompressed size) to a
//! standard zlib deflate stream. Objects written by the Qt build must stay
//! readable here and vice versa.

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

pub fn q_compress(data: &[u8], level: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2 + 8);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut encoder = ZlibEncoder::new(out, Compression::new(level));
    encoder
        .write_all(data)
        .expect("in-memory zlib write cannot fail");
    encoder.finish().expect("in-memory zlib finish cannot fail")
}

pub fn q_uncompress(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 4 {
        return None;
    }
    let expected = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut decoder = ZlibDecoder::new(&data[4..]);
    let mut out = Vec::with_capacity(expected);
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let data = b"hello musit world, hello musit world";
        let compressed = q_compress(data, 6);
        assert_eq!(q_uncompress(&compressed).as_deref(), Some(&data[..]));
    }

    #[test]
    fn known_qt_frame() {
        // qCompress prefixes a big-endian u32 with the uncompressed size.
        let compressed = q_compress(b"abc", 6);
        assert_eq!(&compressed[..4], &[0, 0, 0, 3]);
        // zlib magic follows.
        assert_eq!(compressed[4], 0x78);
    }
}
