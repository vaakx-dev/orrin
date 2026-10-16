use orrin_core::api::PcsSettings;
use orrin_core::model::PcSettings;
use orrin_core::{Error, Result};

use crate::service::Inner;

impl Inner {
    pub fn my_settings(&self) -> PcSettings {
        self.read_book(|b| b.me.clone()).unwrap_or_else(|| self.config.pc.clone())
    }

    pub fn settings(&self, input: PcsSettings) -> Result<PcSettings> {
        let other = input.pc.filter(|pc| *pc != self.me.pc);
        match (other, input.set) {
            (None, Some(set)) => {
                self.edit_book(|b| b.me = Some(set.clone()))?;
                Ok(set)
            }
            (None, None) => Ok(self.my_settings()),
            (Some(pc), None) => Ok(self.peer(&pc)?.settings),
            (Some(pc), Some(_)) => Err(Error::invalid(format!("change the settings of {pc} on that PC"))),
        }
    }
}
