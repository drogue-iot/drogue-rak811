#![no_std]

use embedded_hal::digital::v2::OutputPin;
use embedded_hal::{serial::Read, serial::Write};
use nb;
mod buffer;
mod error;
mod parser;
mod protocol;

use buffer::*;
pub use error::*;
use heapless::consts;
use heapless::spsc::Queue;
pub use protocol::*;

const RECV_BUFFER_LEN: usize = 256;

pub struct Rak811Driver<W, R, RST>
where
    W: Write<u8>,
    R: Read<u8>,
    RST: OutputPin,
{
    tx: W,
    rx: R,
    parse_buffer: Buffer,
    rxq: Queue<Response, consts::U4>,
    connect_mode: ConnectMode,
    lora_mode: LoraMode,
    lora_band: LoraRegion,
    rst: RST,
}

impl<W, R, RST> Rak811Driver<W, R, RST>
where
    W: Write<u8>,
    R: Read<u8>,
    RST: OutputPin,
{
    pub fn new(tx: W, rx: R, rst: RST) -> Result<Rak811Driver<W, R, RST>, DriverError> {
        let mut driver = Rak811Driver {
            tx,
            rx,
            rst,
            parse_buffer: Buffer::new(),
            connect_mode: ConnectMode::OTAA,
            lora_mode: LoraMode::WAN,
            lora_band: LoraRegion::EU868,
            rxq: Queue::new(),
        };

        driver.initialize()?;
        Ok(driver)
    }

    pub fn initialize(&mut self) -> Result<(), DriverError> {
        self.rst.set_low();
        let response = self.recv_response()?;
        match response {
            Response::Initialized => Ok(()),
            r => Err(DriverError::NotInitialized),
        }
    }

    pub fn reset(&mut self, mode: ResetMode) -> Result<(), DriverError> {
        let response = self.send_command(Command::Reset(mode))?;
        match response {
            Response::Ok => {
                let response = self.recv_response()?;
                match response {
                    Response::Initialized => Ok(()),
                    _ => Err(DriverError::NotInitialized),
                }
            }
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    pub fn join(&mut self, mode: ConnectMode) -> Result<(), DriverError> {
        self.connect_mode = mode;
        let response = self.send_command(Command::Join(mode))?;
        match response {
            Response::Ok => {
                let response = self.recv_response()?;
                match response {
                    Response::Recv(EventCode::JoinedSuccess, _, len, _) => Ok(()),
                    r => Err(DriverError::UnexpectedResponse(r)),
                }
            }
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    pub fn set_band(&mut self, band: LoraRegion) -> Result<(), DriverError> {
        self.lora_band = band;
        let response = self.send_command(Command::SetBand(band))?;
        match response {
            Response::Ok => Ok(()),
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
                    Response::Recv(c, 0, _, _) if expected_code == c => Ok(()),
                    r => Err(DriverError::UnexpectedResponse(r)),
                }
            }
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    pub fn try_recv(&mut self, port: Port, rx_buf: &mut [u8]) -> Result<usize, DriverError> {
        self.digest()?;
        let mut tries = self.rxq.len();
        while tries > 0 {
            match self.rxq.dequeue() {
                None => return Ok(0),
                Some(Response::Recv(EventCode::RecvData, p, len, Some(data))) if p == port => {
                    if len > rx_buf.len() {
                        self.rxq
                            .enqueue(Response::Recv(EventCode::RecvData, p, len, Some(data)))
                            .map_err(|_| DriverError::ReadError)?;
                    }

                    rx_buf[0..len].clone_from_slice(&data);
                    return Ok(len);
                }
                Some(event) => {
                    self.rxq
                        .enqueue(event)
                        .map_err(|_| DriverError::ReadError)?;
                }
            }
            tries -= 1;
        }
        Ok(0)
    }

    pub fn process(&mut self) -> Result<(), DriverError> {
        loop {
            match self.rx.read() {
                Err(nb::Error::WouldBlock) => {
                    break;
                }
                Err(nb::Error::Other(_)) => return Err(DriverError::ReadError),
                Ok(b) => {
                    self.parse_buffer
                        .write(b)
                        .map_err(|_| DriverError::ReadError)?;
                }
            }
        }
        Ok(())
    }

    pub fn digest(&mut self) -> Result<(), DriverError> {
        let result = self.parse_buffer.parse();
        if let Ok(response) = result {
            if !matches!(response, Response::None) {
                log::debug!("Got response: {:?}", response);
                self.rxq
                    .enqueue(response)
                    .map_err(|_| DriverError::ReadError)?;
            }
        }
        Ok(())
    }

    fn recv_response(&mut self) -> Result<Response, DriverError> {
        loop {
            for i in 0..1000 {
                self.process()?;
            }
            self.digest()?;
            match self.rxq.dequeue() {
                Some(response) => return Ok(response),
                None => {}
            }
        }
    }

    fn do_write(&mut self, buf: &[u8]) -> Result<(), DriverError> {
        for b in buf.iter() {
            match self.tx.write(*b) {
                Err(nb::Error::WouldBlock) => {
                    nb::block!(self.tx.flush()).map_err(|_| DriverError::WriteError)?;
                }
                Err(_) => return Err(DriverError::WriteError),
                _ => {}
            }
        }
        nb::block!(self.tx.flush()).map_err(|_| DriverError::WriteError)?;
        Ok(())
    }

    pub fn send_command(&mut self, command: Command) -> Result<Response, DriverError> {
        let mut s = Command::buffer();
        command.encode(&mut s);
        log::debug!("Sending command {}", s.as_str());
        self.do_write(s.as_bytes())?;
        self.do_write("\r\n".as_bytes())?;

        let response = self.recv_response()?;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
