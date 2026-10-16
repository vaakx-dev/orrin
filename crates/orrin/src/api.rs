use orrin_core::Home;
use orrin_core::api::Request;
use serde::Serialize;

use crate::client;

pub struct Api {
    pub home: Home,
    pub json: bool,
}

impl Api {
    pub async fn send<R: Request>(&self, input: R) -> anyhow::Result<R::Output> {
        let reply = client::call(&self.home, R::NAME, serde_json::to_value(input)?).await?;
        Ok(serde_json::from_value(reply)?)
    }

    pub fn show<T: Serialize>(&self, value: &T, human: impl FnOnce(&T) -> String) -> anyhow::Result<()> {
        let text = match self.json {
            true => serde_json::to_string_pretty(value)?,
            false => human(value),
        };
        if !text.is_empty() {
            println!("{}", text.trim_end());
        }
        Ok(())
    }
}
