use crate::{AdlSound, OPLSettings};

pub struct OPL {}

pub fn new() -> Result<OPL, &'static str> {
    Ok(OPL {})
}

impl OPL {
    pub fn init(&mut self, settings: OPLSettings) {
        todo!("implement init for web");
    }

    pub fn play_imf(&mut self, data: Vec<u8>) -> Result<(), &'static str> {
        todo!("implement play_imf for web")
    }

    pub fn play_adl(&mut self, sound: AdlSound) -> Result<(), &'static str> {
        todo!("implement play_adl for web")
    }

    pub fn write_reg(&mut self, reg: u32, val: u8) {
        todo!("implement write_reg for web");
    }
}
