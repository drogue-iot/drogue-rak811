#![no_std]

use embedded_hal::{serial::Read, serial::Write};
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
    connect_mode: ConnectMode,
    lora_mode: LoraMode,
}

impl<W, R> Rak811Driver<W, R>
where
    W: Write<u8>,
    R: Read<u8>,
{
    pub fn new(tx: W, rx: R) -> Rak811Driver<W, R> {
        Rak811Driver {
            tx,
            rx,
            connect_mode: ConnectMode::OTAA,
            lora_mode: LoraMode::WAN,
        }
    }

    pub fn join(&mut self, mode: ConnectMode) -> Result<(), DriverError> {
        self.connect_mode = mode;
        let response = self.send_command(Command::Join(mode))?;
        match response {
            Response::Ok => {
                let response = self.recv_response()?;
                match response {
                    Response::Recv(EventCode::JoinedSuccess, _, _) => Ok(()),
                    r => Err(DriverError::UnexpectedResponse(r)),
                }
            }
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    pub fn set_mode(&mut self, mode: LoraMode) -> Result<(), DriverError> {
        self.lora_mode = mode;
        let response = self.send_command(Command::SetMode(mode))?;
        match response {
            Response::Ok => Ok(()),
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    pub fn set_device_address(&mut self, addr: &DevAddr) -> Result<(), DriverError> {
        let response = self.send_command(Command::SetConfig(ConfigOption::DevAddr(addr)))?;
        match response {
            Response::Ok => Ok(()),
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }
    pub fn set_device_eui(&mut self, eui: &EUI) -> Result<(), DriverError> {
        let response = self.send_command(Command::SetConfig(ConfigOption::DevEui(eui)))?;
        match response {
            Response::Ok => Ok(()),
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }
    pub fn set_app_eui(&mut self, eui: &EUI) -> Result<(), DriverError> {
        let response = self.send_command(Command::SetConfig(ConfigOption::AppEui(eui)))?;
        match response {
            Response::Ok => Ok(()),
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    pub fn set_app_key(&mut self, key: &AppKey) -> Result<(), DriverError> {
        let response = self.send_command(Command::SetConfig(ConfigOption::AppKey(key)))?;
        match response {
            Response::Ok => Ok(()),
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    pub fn send(&mut self, qos: QoS, port: Port, data: &[u8]) -> Result<(), DriverError> {
        let response = self.send_command(Command::Send(qos, port, data))?;
        match response {
            Response::Ok => {
                let response = self.recv_response()?;
                let expected_code = match qos {
                    QoS::Unconfirmed => EventCode::TxUnconfirmed,
                    QoS::Confirmed => EventCode::TxConfirmed,
                };
                match response {
                    Response::Recv(c, 0, _) if expected_code == c => Ok(()),
                    r => Err(DriverError::UnexpectedResponse(r)),
                }
            }
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    pub fn send_command(&mut self, command: Command) -> Result<Response, DriverError> {
        let mut s = Command::buffer();
        command.encode(&mut s);
        log::info!("Sending command {}", s.as_str());
        for b in s.as_bytes().iter() {
            nb::block!(self.tx.write(*b)).map_err(|_| DriverError::WriteError)?;
        }
        nb::block!(self.tx.write(b'\r')).map_err(|_| DriverError::WriteError)?;
        nb::block!(self.tx.write(b'\n')).map_err(|_| DriverError::WriteError)?;

        let response = self.recv_response()?;
        Ok(response)
    }

    pub fn recv_response(&mut self) -> Result<Response, DriverError> {
        let mut res = [0; 64];
        let mut rp = 0;
        loop {
            let mut try_parse = false;
            loop {
                let b = nb::block!(self.rx.read()).map_err(|_| DriverError::ReadError)?;
                res[rp] = b;
                rp += 1;
                if b as char == '\n' {
                    try_parse = true;
                    break;
                }
            }

            if try_parse {
                //log::info!("Res bytes: {:?}", &res[..rp]);

                if let Ok((_remainder, response)) = parser::parse(&res[..rp]) {
                    return Ok(response);
                } else {
                    if let Ok(msg) = core::str::from_utf8(&res[..rp]) {
                        log::info!("Partial res: {}", msg);
                        log::info!("Partial bytes: {:?}", &res[..rp]);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
