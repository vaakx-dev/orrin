use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use orrin_core::{Error, PcId, Result};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "orrin-";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairCode {
    pub pc: PcId,
    pub name: String,
    pub addresses: Vec<String>,
    pub secret: String,
}

impl PairCode {
    pub fn encode(&self) -> String {
        let json = serde_json::to_vec(self).unwrap_or_default();
        format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
    }

    pub fn decode(code: &str) -> Result<Self> {
        let bad = || Error::invalid("not an orrin pairing code");
        let body = code.trim().strip_prefix(PREFIX).ok_or_else(bad)?;
        let json = URL_SAFE_NO_PAD.decode(body).map_err(|_| bad())?;
        serde_json::from_slice(&json).map_err(|_| bad())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let code = PairCode {
            pc: PcId::from("pc_a"),
            name: "desk".into(),
            addresses: vec!["10.0.0.2:4520".into()],
            secret: "s".into(),
        };
        assert_eq!(PairCode::decode(&code.encode()).unwrap(), code);
        assert!(PairCode::decode("orrin-???").is_err());
    }
}
