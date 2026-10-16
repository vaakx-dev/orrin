use std::fmt::Display;
use std::io::{Cursor, Read, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::Parts;

fn broken(error: impl Display) -> String {
    format!("not a valid package: {error}")
}

pub fn read(bytes: &[u8]) -> Result<Parts, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(broken)?;
    let mut parts = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(broken)?;
        if file.is_dir() {
            continue;
        }
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(broken)?;
        parts.push((file.name().map_err(broken)?.into_owned(), data));
    }
    Ok(parts)
}

pub fn write(parts: &Parts) -> Result<Vec<u8>, String> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, data) in parts {
        zip.start_file(name.as_str(), options).map_err(broken)?;
        zip.write_all(data).map_err(broken)?;
    }
    Ok(zip.finish().map_err(broken)?.into_inner())
}
