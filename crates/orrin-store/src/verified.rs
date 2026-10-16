use std::io::{self, Write};

use orrin_core::{Error, Hash, Result};

pub struct Verified<W> {
    inner: W,
    hasher: blake3::Hasher,
}

impl<W: Write> Verified<W> {
    pub fn new(inner: W) -> Self {
        Self { inner, hasher: blake3::Hasher::new() }
    }

    pub fn finish(mut self, hash: &Hash) -> Result<W> {
        self.inner.flush()?;
        if Hash::from_blake3(self.hasher.finalize()) != *hash {
            return Err(Error::internal(format!("object {hash} rebuilt with the wrong content")));
        }
        Ok(self.inner)
    }
}

impl<W: Write> Write for Verified<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.hasher.update(&buf[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
