use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use orrin_core::{Error, Result};

pub struct Claims(GlobSet);

impl Claims {
    pub fn new(claims: &[String]) -> Result<Self> {
        let mut set = GlobSetBuilder::new();
        for claim in claims {
            let claim = claim.trim_end_matches('/');
            for pattern in [claim.to_string(), format!("{claim}/**")] {
                let glob = GlobBuilder::new(&pattern).literal_separator(true).build().map_err(bad_claim)?;
                set.add(glob);
            }
        }
        Ok(Self(set.build().map_err(bad_claim)?))
    }

    pub fn covers(&self, path: &str) -> bool {
        self.0.is_match(path)
    }
}

fn bad_claim(err: globset::Error) -> Error {
    Error::invalid(format!("bad claim: {err}"))
}
