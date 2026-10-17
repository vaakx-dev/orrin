use std::collections::BTreeMap;

use orrin_core::{Hash, Result};
use serde::Serialize;

#[derive(Serialize)]
pub struct Base<'a> {
    pub command: &'a str,
    pub env: &'a BTreeMap<String, String>,
    pub tools: &'a [(&'a str, Option<&'a str>)],
    pub inputs: &'a [(&'a str, &'a Hash)],
    pub out: Option<&'a str>,
}

#[derive(Serialize)]
struct Parts<'a> {
    os: &'static str,
    arch: &'static str,
    base: &'a Base<'a>,
}

pub fn base(base: &Base) -> Result<Hash> {
    let parts = Parts { os: std::env::consts::OS, arch: std::env::consts::ARCH, base };
    Ok(Hash::of(&serde_json::to_vec(&parts)?))
}

pub fn full(base: &Hash, deps: &[(String, Hash)]) -> Result<Hash> {
    Ok(Hash::of(&serde_json::to_vec(&(base, deps))?))
}
