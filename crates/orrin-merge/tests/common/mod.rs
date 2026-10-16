#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use orrin_core::service::Merged;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

pub fn merge(path: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Merged {
    let merger = orrin_merge::all().into_iter().find(|merger| merger.handles(path)).expect("a merger");
    merger.merge(path, base, ours, theirs)
}

pub fn clean(merged: Merged) -> Vec<u8> {
    match merged {
        Merged::Clean(bytes) => bytes,
        Merged::Conflict(reason) => panic!("unexpected conflict: {reason}"),
    }
}

pub fn clean_text(merged: Merged) -> String {
    String::from_utf8(clean(merged)).unwrap()
}

pub fn zip(parts: &[(String, String)]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in parts {
        zip.start_file(name.as_str(), SimpleFileOptions::default()).unwrap();
        zip.write_all(data.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

pub fn unzip(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    (0..archive.len())
        .map(|index| {
            let mut file = archive.by_index(index).unwrap();
            let mut data = String::new();
            file.read_to_string(&mut data).unwrap();
            (file.name().unwrap().into_owned(), data)
        })
        .collect()
}
