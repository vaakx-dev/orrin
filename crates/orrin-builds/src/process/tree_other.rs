use orrin_core::Result;
use tokio::process::{Child, Command};

use super::Limits;

pub struct Tree;

impl Tree {
    pub fn spawn(command: &mut Command, _limits: Limits) -> Result<(Child, Tree)> {
        Ok((command.spawn()?, Tree))
    }

    pub fn kill(&self) {}
}
