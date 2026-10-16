use std::path::Path;

use crate::error::{Error, Result};
use crate::home::Home;
use crate::secret::write_private;

#[derive(Clone, Debug)]
pub struct Cert {
    pub der: Vec<u8>,
    pub key: Vec<u8>,
    pub fingerprint: String,
}

impl Cert {
    pub fn load_or_create(home: &Home) -> Result<Self> {
        let dir = home.dir("tls")?;
        let (cert_path, key_path) = (dir.join("cert.der"), dir.join("key.der"));
        if !cert_path.exists() || !key_path.exists() {
            create(&cert_path, &key_path)?;
        }
        let der = std::fs::read(&cert_path)?;
        Ok(Self { fingerprint: fingerprint(&der), der, key: std::fs::read(&key_path)? })
    }
}

pub fn fingerprint(der: &[u8]) -> String {
    blake3::hash(der).to_hex().to_string()
}

fn create(cert_path: &Path, key_path: &Path) -> Result<()> {
    let made = rcgen::generate_simple_self_signed(vec!["orrin".to_string()])
        .map_err(|e| Error::internal(format!("cannot make a certificate: {e}")))?;
    write_private(key_path, &made.signing_key.serialize_der())?;
    std::fs::write(cert_path, made.cert.der())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_once() {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path()).unwrap();
        let first = Cert::load_or_create(&home).unwrap();
        let again = Cert::load_or_create(&home).unwrap();
        assert_eq!(first.fingerprint, again.fingerprint);
        assert_eq!(first.fingerprint, fingerprint(&again.der));
    }
}
