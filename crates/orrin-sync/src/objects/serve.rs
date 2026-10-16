use orrin_core::api::{Hashes, Objects, Stored, SyncGet, SyncMissing, SyncPut};
use orrin_core::model::Keep;
use orrin_core::{Error, Result};

use super::{pack, unpack};
use crate::service::Inner;

const REPLY_BYTES: usize = 16 << 20;

impl Inner {
    pub fn missing(&self, input: SyncMissing) -> Hashes {
        Hashes { hashes: self.hub.store.missing(&input.0.hashes) }
    }

    pub fn objects(&self, input: SyncGet) -> Objects {
        let mut objects = Vec::new();
        let mut bytes = 0;
        for hash in input.0.hashes {
            if bytes >= REPLY_BYTES {
                break;
            }
            let Ok(raw) = self.hub.store.raw(&hash) else { continue };
            bytes += raw.len();
            objects.push(pack(hash, &raw));
        }
        Objects { objects }
    }

    pub fn receive(&self, input: SyncPut) -> Result<Stored> {
        if self.my_settings().keep == Keep::None {
            return Err(Error::unavailable("this PC keeps no project copies"));
        }
        let mut stored = 0;
        for object in &input.0.objects {
            self.hub.store.put_raw(&object.hash, &unpack(object)?)?;
            stored += 1;
        }
        Ok(Stored { stored })
    }
}
