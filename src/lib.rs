#![no_std]

use core::fmt::Write;
use hal::pac::UARTE0;
use hal::uarte::*;
use nrf52833_hal as hal;

pub struct Rak811Driver {
    uarte: Uarte<UARTE0>,
}

impl Rak811Driver {
    fn new(uarte: Uarte<UARTE0>) -> Rak811Driver {
        Rak811Driver { uarte }
    }

    pub fn send(&mut self, command: Command) -> Result<Response, AdapterError> {
        log::info!("Writing command {:?}", command);
        command.write(&mut self.uarte)?;
        Ok(Response::Ok)
    }

    pub fn on_interrupt(&mut self) -> Result<(), u8> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum Command {
    QueryAtVersion,
}

impl Command {
    pub fn write<W: core::fmt::Write>(&self, w: &mut W) -> Result<(), AdapterError> {
        match self {
            Command::QueryAtVersion => {
                w.write_str("AT+VERSION\r\n")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Response {
    Ok,
}

/*

pub trait LoraDriver {
    fn send(&mut self, data: &[u8]) -> Result<usize, AdapterError>;
    fn recv(&mut self, data: &mut [u8]) -> Result<usize, AdapterError>;
}
impl LoraDriver for Rak811Driver {
    fn send(&mut self, data: &[u8]) -> Result<usize, AdapterError> {
        //  self.uarte.write_str("at+send=0,1,");
        //  self.uarte.write(
        Ok()
    }

    fn recv(&mut self, data: &mut [u8]) -> Result<usize, AdapterError> {
        Ok(0)
    }
}*/

pub enum AdapterError {
    WriteError,
}

impl core::convert::From<core::fmt::Error> for AdapterError {
    fn from(error: core::fmt::Error) -> Self {
        AdapterError::WriteError
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
