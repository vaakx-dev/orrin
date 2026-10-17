use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use orrin_core::{Error, Hash, Result};

const PART: &str = ".part";
const LONGEST: usize = 200;

pub struct Keys {
    dir: PathBuf,
}

pub struct Key {
    pub name: String,
    pub hash: Hash,
    pub used: SystemTime,
}

impl Keys {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn find(&self, key: &str) -> Option<Hash> {
        let path = self.dir.join(checked(key).ok()?);
        let hash = Hash::parse(fs::read_to_string(&path).ok()?.trim()).ok()?;
        let _ = fs::File::options().write(true).open(&path).and_then(|file| file.set_modified(SystemTime::now()));
        Some(hash)
    }

    pub fn put(&self, key: &str, hash: &Hash) -> Result<()> {
        let path = self.dir.join(checked(key)?);
        let part = self.dir.join(format!("{key}{PART}"));
        fs::write(&part, hash.as_str())?;
        fs::rename(part, path)?;
        Ok(())
    }

    pub fn forget(&self, key: &str) {
        let _ = fs::remove_file(self.dir.join(key));
    }

    pub fn all(&self) -> Vec<Key> {
        let entries = fs::read_dir(&self.dir).into_iter().flatten().flatten();
        entries.filter_map(|entry| read(&entry)).collect()
    }
}

fn read(entry: &fs::DirEntry) -> Option<Key> {
    let name = entry.file_name().into_string().ok().filter(|name| !name.ends_with(PART))?;
    let hash = Hash::parse(fs::read_to_string(entry.path()).ok()?.trim()).ok()?;
    let used = entry.metadata().ok()?.modified().ok()?;
    Some(Key { name, hash, used })
}

pub fn checked(key: &str) -> Result<&str> {
    let allowed = |b: u8| b.is_ascii_alphanumeric() || b"._-".contains(&b);
    let fits = key.len() <= LONGEST && key.bytes().all(allowed) && key.bytes().any(|b| b.is_ascii_alphanumeric());
    if fits { Ok(key) } else { Err(Error::invalid(format!("not a cache key: {key}"))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_keys_and_rejects_paths() {
        let dir = tempfile::tempdir().unwrap();
        let keys = Keys::new(dir.path().to_path_buf());
        let hash = Hash::of(b"entry");
        keys.put("abc123", &hash).unwrap();
        assert_eq!(keys.find("abc123"), Some(hash));
        assert_eq!(keys.all().len(), 1);
        assert!(keys.put("../x", &Hash::of(b"x")).is_err());
        assert!(checked("..").is_err());
        keys.forget("abc123");
        assert!(keys.find("abc123").is_none());
    }
}
