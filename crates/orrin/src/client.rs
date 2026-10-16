use orrin_core::daemon::DaemonInfo;
use orrin_core::{Error, Home};
use serde_json::Value;

pub async fn call(home: &Home, request: &str, body: Value) -> anyhow::Result<Value> {
    let daemon = DaemonInfo::read(home)?;
    let url = format!("{}/api/{request}", daemon.url());
    let reply: Value = reqwest::Client::new().post(url).json(&body).send().await?.json().await?;
    if reply["ok"] == Value::Bool(true) {
        return Ok(reply["result"].clone());
    }
    let error: Error = serde_json::from_value(reply["error"].clone())?;
    Err(error.into())
}
