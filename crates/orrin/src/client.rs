use orrin_core::daemon::{DaemonInfo, LOCAL_KEY_HEADER};
use orrin_core::{Error, Home};
use serde_json::Value;

pub async fn call(home: &Home, request: &str, body: Value) -> anyhow::Result<Value> {
    orrin_server::install_crypto();
    let daemon = DaemonInfo::read(home)?;
    let url = format!("{}/api/{request}", daemon.url());
    let request = reqwest::Client::new().post(url).header(LOCAL_KEY_HEADER, home.local_key()?).json(&body);
    let reply: Value = request.send().await?.json().await?;
    if reply["ok"] == Value::Bool(true) {
        return Ok(reply["result"].clone());
    }
    let error: Error = serde_json::from_value(reply["error"].clone())?;
    Err(error.into())
}
