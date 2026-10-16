use orrin_core::api::Request;
use orrin_core::{Error, PcId, Result};
use serde_json::Value;

use crate::address::url;
use crate::service::Inner;

const PC_HEADER: &str = "x-orrin-pc";
const KEY_HEADER: &str = "x-orrin-key";

impl Inner {
    pub async fn call(&self, pc: &PcId, request: &str, body: Value) -> Result<Value> {
        let peer = self.peer(pc)?;
        self.post(&peer.address, &peer.fingerprint, request, &body, Some(&peer.send_key)).await
    }

    pub async fn ask<R: Request>(&self, pc: &PcId, input: R) -> Result<R::Output> {
        let value = self.call(pc, R::NAME, serde_json::to_value(input)?).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn post(
        &self,
        address: &str,
        fingerprint: &str,
        request: &str,
        body: &Value,
        key: Option<&str>,
    ) -> Result<Value> {
        let mut builder = self.transport.client(fingerprint)?.post(url(address, request)).json(body);
        if let Some(key) = key {
            builder = builder.header(PC_HEADER, self.me.pc.as_str()).header(KEY_HEADER, key);
        }
        let unreachable = |e: reqwest::Error| Error::unavailable(format!("{address}: {e}"));
        let reply: Value = builder.send().await.map_err(unreachable)?.json().await.map_err(unreachable)?;
        unwrap_reply(reply)
    }
}

fn unwrap_reply(mut reply: Value) -> Result<Value> {
    if reply["ok"] == Value::Bool(true) {
        return Ok(reply["result"].take());
    }
    Err(serde_json::from_value(reply["error"].take()).unwrap_or_else(|_| Error::internal("bad reply from peer")))
}

#[cfg(test)]
mod tests {
    use orrin_core::ErrorCode;
    use serde_json::json;

    use super::*;

    #[test]
    fn unwraps_replies() {
        assert_eq!(unwrap_reply(json!({ "ok": true, "result": 3 })).unwrap(), json!(3));
        let err = unwrap_reply(json!({ "ok": false, "error": { "code": "conflict", "message": "m" } })).unwrap_err();
        assert_eq!(err.code, ErrorCode::Conflict);
    }
}
