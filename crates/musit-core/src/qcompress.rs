//! Qt `qCompress`/`qUncompress`-compatible framing.
//!
//! Qt prepends a 4-byte big-endian length (the uncompressed size) to a
//! standard zlib deflate stream. Objects written by the Qt build must stay
//! readable here and vice versa.

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::io::{Read, Write};

pub fn q_compress(data: &[u8], level: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2 + 8);
    // Saturate rather than wrap for inputs >= 4 GiB; readers treat the
    // prefix as a hint only.
    let prefix = u32::try_from(data.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&prefix.to_be_bytes());
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
    // Qt's qCompress emits exactly 4 zero bytes (no zlib stream) for empty
    // input, and qUncompress accepts that frame.
    if data.len() == 4 {
        return (expected == 0).then(Vec::new);
    }
    let mut decoder = ZlibDecoder::new(&data[4..]);
    // The prefix is untrusted (read from disk); use it as a capacity hint
    // only up to a sane bound so a corrupt frame can't force a huge alloc.
    let mut out = Vec::with_capacity(expected.min(64 * 1024 * 1024));
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

/// Streaming counterpart of [`q_compress`]: frames whatever is written into
/// it (prefix from `uncompressed_len`, then the zlib stream) into `out`, so
/// large objects never need a whole-file buffer. `uncompressed_len` must be
/// the total number of bytes that will be written.
pub struct QCompressWriter<W: Write> {
    encoder: ZlibEncoder<W>,
}

impl<W: Write> QCompressWriter<W> {
    pub fn new(mut out: W, uncompressed_len: u64, level: u32) -> std::io::Result<Self> {
        // Saturate rather than wrap for inputs >= 4 GiB, like q_compress;
        // readers treat the prefix as a hint only.
        let prefix = u32::try_from(uncompressed_len).unwrap_or(u32::MAX);
        out.write_all(&prefix.to_be_bytes())?;
        Ok(Self {
            encoder: ZlibEncoder::new(out, Compression::new(level)),
        })
    }

    /// Finishes the zlib stream and hands back the underlying writer.
    pub fn finish(self) -> std::io::Result<W> {
        self.encoder.finish()
    }
}

impl<W: Write> Write for QCompressWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.encoder.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.encoder.flush()
    }
}

/// Streaming counterpart of [`q_uncompress`]: decodes a qCompress frame from
/// `input` as it is read, in constant memory. Returns `None` for a frame too
/// short to carry the length prefix, or a bare 4-byte frame whose declared
/// size is nonzero (mirroring `q_uncompress` on the same bytes).
pub struct QUncompressReader<R: Read> {
    // `None` is Qt's empty frame (exactly 4 zero bytes, no zlib stream).
    decoder: Option<ZlibDecoder<std::io::Chain<std::io::Cursor<[u8; 1]>, R>>>,
}

impl<R: Read> QUncompressReader<R> {
    pub fn new(mut input: R) -> Option<Self> {
        let mut prefix = [0u8; 4];
        input.read_exact(&mut prefix).ok()?;
        let expected = u32::from_be_bytes(prefix);
        // Distinguish the bare empty frame from a frame with a zlib stream:
        // probe one byte, then stitch it back in front of the stream.
        let mut first = [0u8; 1];
        match input.read(&mut first) {
            Ok(0) => return (expected == 0).then_some(Self { decoder: None }),
            Ok(_) => {}
            Err(_) => return None,
        }
        let stitched = std::io::Cursor::new(first).chain(input);
        Some(Self {
            decoder: Some(ZlibDecoder::new(stitched)),
        })
    }
}

impl<R: Read> Read for QUncompressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.decoder {
            Some(decoder) => decoder.read(buf),
            None => Ok(0),
        }
    }
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
    fn qt_empty_frame() {
        // Qt's qCompress of empty input is exactly 4 zero bytes.
        assert_eq!(q_uncompress(&[0, 0, 0, 0]).as_deref(), Some(&[][..]));
        // A nonzero prefix with no stream is corrupt, not empty.
        assert_eq!(q_uncompress(&[0, 0, 0, 5]), None);
        // Rust-written empty frames must stay readable too.
        let compressed = q_compress(b"", 6);
        assert_eq!(q_uncompress(&compressed).as_deref(), Some(&[][..]));
    }

    #[test]
    fn known_qt_frame() {
        // qCompress prefixes a big-endian u32 with the uncompressed size.
        let compressed = q_compress(b"abc", 6);
        assert_eq!(&compressed[..4], &[0, 0, 0, 3]);
        // zlib magic follows.
        assert_eq!(compressed[4], 0x78);
    }

    #[test]
    fn streaming_writer_matches_in_memory_frames() {
        let data = b"streaming and in-memory frames must stay interchangeable";
        let mut framed: Vec<u8> = Vec::new();
        let mut writer = QCompressWriter::new(&mut framed, data.len() as u64, 6).unwrap();
        writer.write_all(data).unwrap();
        writer.finish().unwrap();
        assert_eq!(q_uncompress(&framed).as_deref(), Some(&data[..]));

        // And the reverse: streaming reader over an in-memory frame.
        let compressed = q_compress(data, 6);
        let mut reader = QUncompressReader::new(&compressed[..]).unwrap();
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn streaming_reader_handles_empty_frames() {
        // Qt's bare empty frame.
        let mut reader = QUncompressReader::new(&[0u8, 0, 0, 0][..]).unwrap();
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert!(out.is_empty());
        // A nonzero prefix with no stream is corrupt, not empty.
        assert!(QUncompressReader::new(&[0u8, 0, 0, 5][..]).is_none());
        // Too short to carry a prefix.
        assert!(QUncompressReader::new(&[0u8, 0][..]).is_none());
        // Rust-written empty frames (prefix + empty zlib stream) decode too.
        let compressed = q_compress(b"", 6);
        let mut reader = QUncompressReader::new(&compressed[..]).unwrap();
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert!(out.is_empty());
    }
}
