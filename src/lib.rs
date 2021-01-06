#![no_std]

use embedded_hal::{digital::v2::OutputPin, serial::Read, serial::Write};
use nb;
mod error;
mod parser;
mod protocol;

pub use error::*;
pub use protocol::*;

pub struct Rak811Driver<W, R>
where
    W: Write<u8>,
    R: Read<u8>,
{
    tx: W,
    rx: R,
}

impl<W, R> Rak811Driver<W, R>
where
    W: Write<u8>,
    R: Read<u8>,
{
    pub fn new(tx: W, rx: R) -> Rak811Driver<W, R> {
        Rak811Driver { tx, rx }
    }

    pub fn send(&mut self, command: Command) -> Result<Response, DriverError> {
        command.write(&mut self.tx)?;

        let response = self.recv()?;
        Ok(response)
    }

    pub fn recv(&mut self) -> Result<Response, DriverError> {
        let mut res = [0; 64];
        let mut rp = 0;
        loop {
            let b = nb::block!(self.rx.read()).map_err(|_| DriverError::ReadError)?;
            res[rp] = b;
            rp += 1;
            if b as char == '\n' {
                break;
            }
        }

        let mut ret = Err(DriverError::ReadError);
        if rp > 0 {
            if let Ok(msg) = core::str::from_utf8(&res[..rp]) {
                log::trace!("Res: {}", msg);
            }
            match parser::parse(&res[..rp]) {
                Ok((_remainder, response)) => {
                    ret = Ok(response);
                }
                Err(e) => {
                    log::info!("Error parse: {:?}", e);
                }
            }
        }

        ret
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
