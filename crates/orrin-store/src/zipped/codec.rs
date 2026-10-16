use std::io::{self, Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;

const INFLATE_LIMIT: u64 = 1024 * 1024 * 1024;

pub fn inflate(raw: &[u8]) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    DeflateDecoder::new(raw).take(INFLATE_LIMIT).read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn deflate(bytes: &[u8], level: u32) -> io::Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(level));
    encoder.write_all(bytes)?;
    encoder.finish()
}

pub fn level_hint(flags: u16) -> u32 {
    match (flags >> 1) & 0b11 {
        0 => 6,
        1 => 9,
        _ => 1,
    }
}
