use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::Write;

pub struct EventCompressor;

impl EventCompressor {
    pub fn compress(data: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap_or_default()
    }

    pub fn decompress(data: &[u8]) -> Vec<u8> {
        use flate2::read::DeflateDecoder;
        let mut decoder = DeflateDecoder::new(data);
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut out).ok();
        out
    }
}
