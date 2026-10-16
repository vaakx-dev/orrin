use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use orrin_core::api::Request;
use orrin_core::service::Hub;
use orrin_core::{Error, Result};
use serde_json::Value;

type Reply = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;
type Handler = Box<dyn Fn(Hub, Value) -> Reply + Send + Sync>;

#[derive(Default)]
pub struct Registry {
    handlers: HashMap<&'static str, Handler>,
}

impl Registry {
    pub fn add<R, F, Fut>(&mut self, handler: F)
    where
        R: Request,
        F: Fn(Hub, R) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R::Output>> + Send + 'static,
    {
        let erased: Handler = Box::new(move |hub, body| match serde_json::from_value::<R>(body) {
            Ok(input) => {
                let reply = handler(hub, input);
                Box::pin(async move { serde_json::to_value(reply.await?).map_err(|e| Error::internal(e.to_string())) })
            }
            Err(err) => Box::pin(std::future::ready(Err(Error::invalid(format!("{}: {err}", R::NAME))))),
        });
        if self.handlers.insert(R::NAME, erased).is_some() {
            panic!("request {} registered twice", R::NAME);
        }
    }

    pub async fn call(&self, hub: Hub, name: &str, body: Value) -> Result<Value> {
        let handler = self.handlers.get(name).ok_or_else(|| Error::not_found(format!("no request named {name}")))?;
        handler(hub, body).await
    }

    pub fn names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.handlers.keys().copied().collect();
        names.sort();
        names
    }
}
