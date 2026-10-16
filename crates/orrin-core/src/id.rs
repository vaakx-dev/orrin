use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::clock::now_ms;
use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hash(String);

impl Hash {
    pub fn of(bytes: &[u8]) -> Self {
        Self(blake3::hash(bytes).to_hex().to_string())
    }

    pub fn from_blake3(hash: blake3::Hash) -> Self {
        Self(hash.to_hex().to_string())
    }

    pub fn parse(text: &str) -> Result<Self> {
        let valid = text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if valid { Ok(Self(text.to_string())) } else { Err(Error::invalid(format!("not a hash: {text}"))) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub type SnapshotId = Hash;

macro_rules! string_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(fresh($prefix))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(text: &str) -> Self {
                Self(text.to_string())
            }
        }
    };
}

string_id!(ProjectId, "p");
string_id!(ChangeId, "c");
string_id!(ViewId, "v");
string_id!(JobId, "j");
string_id!(PcId, "pc");

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn fresh(prefix: &str) -> String {
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let seed = format!("{}:{}:{}", now_ms(), std::process::id(), count);
    let hash = blake3::hash(seed.as_bytes()).to_hex();
    format!("{prefix}_{}", &hash[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_prefixed() {
        let a = ProjectId::new();
        let b = ProjectId::new();
        assert_ne!(a, b);
        assert!(a.as_str().starts_with("p_"));
    }

    #[test]
    fn hash_parses_its_own_output() {
        let hash = Hash::of(b"hello");
        assert_eq!(Hash::parse(hash.as_str()).unwrap(), hash);
        assert!(Hash::parse("nope").is_err());
    }
}
