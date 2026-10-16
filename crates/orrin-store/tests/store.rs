use std::io::Write;
use std::path::Path;
use std::time::Duration;

use orrin_core::Hash;
use orrin_core::model::{Entry, Snapshot};
use orrin_core::service::{Materialise, Store};
use orrin_store::DiskStore;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn store(dir: &Path) -> DiskStore {
    DiskStore::open(dir.join("store")).unwrap().with_grace(Duration::ZERO)
}

fn noise(seed: &str, len: usize) -> Vec<u8> {
    let mut bytes = vec![0; len];
    blake3::Hasher::new().update(seed.as_bytes()).finalize_xof().fill(&mut bytes);
    bytes
}

fn docx(body: &str) -> Vec<u8> {
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("[Content_Types].xml", deflated).unwrap();
    zip.write_all(b"<Types><Default Extension=\"xml\"/></Types>").unwrap();
    zip.start_file("word/document.xml", deflated).unwrap();
    zip.write_all(body.as_bytes()).unwrap();
    zip.start_file("word/media/image1.bin", stored).unwrap();
    zip.write_all(&noise("image", 20_000)).unwrap();
    zip.finish().unwrap().into_inner()
}

fn entry(path: &str, hash: &Hash) -> Entry {
    Entry { path: path.into(), hash: hash.clone(), size: 1, mode: Default::default() }
}

#[test]
fn small_files_round_trip_and_dedupe() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let hash = store.put(b"hello").unwrap();
    assert_eq!(hash, Hash::of(b"hello"));
    assert_eq!(store.put(b"hello").unwrap(), hash);
    assert_eq!(store.get(&hash).unwrap(), b"hello");
    assert_eq!(store.size(&hash).unwrap(), 5);
    assert!(store.parts(&hash).unwrap().is_empty());
    let other = Hash::of(b"other");
    assert_eq!(store.missing(&[hash.clone(), other.clone()]), vec![other.clone()]);
    assert_eq!(store.get(&other).unwrap_err().code, orrin_core::ErrorCode::NotFound);
}

#[test]
fn big_files_are_chunked_and_materialised() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let bytes = noise("big", 3 * 1024 * 1024);
    let source = dir.path().join("big.bin");
    std::fs::write(&source, &bytes).unwrap();
    let hash = store.put_file(&source).unwrap();
    assert_eq!(hash, Hash::of(&bytes));
    assert_eq!(store.get(&hash).unwrap(), bytes);
    assert_eq!(store.size(&hash).unwrap(), bytes.len() as u64);
    let parts = store.parts(&hash).unwrap();
    assert!(parts.len() > 10);

    let mut edited = bytes.clone();
    edited[1_500_000] ^= 1;
    let changed = store.put(&edited).unwrap();
    let shared = store.parts(&changed).unwrap().iter().filter(|part| parts.contains(part)).count();
    assert!(shared + 2 >= parts.len());

    for (name, how) in [("copy", Materialise::Copy), ("ro", Materialise::ReadonlyCopy), ("link", Materialise::Link)] {
        let target = dir.path().join("out").join(name);
        store.materialise(&hash, &target, how).unwrap();
        store.materialise(&hash, &target, how).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        assert_eq!(std::fs::metadata(&target).unwrap().permissions().readonly(), how != Materialise::Copy);
    }
    let target = dir.path().join("out").join("link");
    store.materialise(&changed, &target, Materialise::Copy).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), edited);
    store.materialise(&hash, &dir.path().join("again"), Materialise::Link).unwrap();
    assert_eq!(std::fs::read(dir.path().join("again")).unwrap(), bytes);
    assert!(std::fs::metadata(dir.path().join("again")).unwrap().permissions().readonly());
}

#[test]
fn docx_is_unpacked_into_parts_and_rebuilt_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let body = "<w:document>".to_string() + &"<w:p>hello</w:p>".repeat(200) + "</w:document>";
    let bytes = docx(&body);
    let hash = store.put(&bytes).unwrap();
    assert_eq!(store.get(&hash).unwrap(), bytes);
    assert_eq!(store.size(&hash).unwrap(), bytes.len() as u64);
    assert!(store.has(&Hash::of(body.as_bytes())));
    assert!(store.parts(&hash).unwrap().contains(&Hash::of(body.as_bytes())));

    let copy = DiskStore::open(dir.path().join("peer")).unwrap();
    copy.put_raw(&hash, &store.raw(&hash).unwrap()).unwrap();
    for part in store.parts(&hash).unwrap() {
        copy.put_raw(&part, &store.raw(&part).unwrap()).unwrap();
    }
    assert_eq!(copy.get(&hash).unwrap(), bytes);
    assert!(copy.put_raw(&Hash::of(b"x"), b"y").is_err());
}

#[test]
fn gc_keeps_what_roots_reach() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let kept = store.put(&noise("kept", 2 * 1024 * 1024)).unwrap();
    let dropped = store.put(b"dropped").unwrap();
    let snapshot = store.put_snapshot(&Snapshot::new(vec![entry("a.bin", &kept)])).unwrap();
    store.materialise(&dropped, &dir.path().join("dropped"), Materialise::Link).unwrap();

    let report = store.gc(std::slice::from_ref(&snapshot)).unwrap();
    assert_eq!(report.removed, 2);
    assert!(!store.has(&dropped));
    assert!(store.has(&snapshot));
    assert_eq!(store.get(&kept).unwrap().len(), 2 * 1024 * 1024);
    assert_eq!(std::fs::read(dir.path().join("dropped")).unwrap(), b"dropped");
}
