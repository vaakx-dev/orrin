use std::io::Write;
use std::path::Path;

use crate::error::{Error, Result};

pub fn random_hex(bytes: usize) -> Result<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| Error::internal(format!("no randomness: {e}")))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_and_compare() {
        let a = random_hex(32).unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, random_hex(32).unwrap());
        assert!(same(&a, &a.clone()));
        assert!(!same(&a, "nope"));
    }
}
