#![no_std]
//! Driver for RAK811 AT-command firmware over UART.

use embedded_hal::digital::v2::OutputPin;
use embedded_hal::{serial::Read, serial::Write};
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
    /// Create a new instance of the driver. The driver will trigger a reset of the module
    /// and expect a response from the firmware.
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

    /// Initialize the driver. This will cause the RAK811 module to be reset.
    pub fn initialize(&mut self) -> Result<(), DriverError> {
        self.rst.set_high().ok();
        self.rst.set_low().ok();
        let response = self.recv_response()?;
        match response {
            Response::Initialized => Ok(()),
            _ => Err(DriverError::NotInitialized),
        }
    }

    /// Send reset command to lora module. Depending on the mode, this will restart
    /// the module or reload its configuration from EEPROM.
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

    /// Join a LoRa Network using the specified mode.
    pub fn join(&mut self, mode: ConnectMode) -> Result<(), DriverError> {
        self.connect_mode = mode;
        let response = self.send_command(Command::Join(mode))?;
        match response {
            Response::Ok => {
                let response = self.recv_response()?;
                match response {
                    Response::Recv(EventCode::JoinedSuccess, _, _, _) => Ok(()),
                    r => Err(DriverError::UnexpectedResponse(r)),
                }
            }
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    /// Set the frequency band based on the region.
    pub fn set_band(&mut self, band: LoraRegion) -> Result<(), DriverError> {
        self.lora_band = band;
        let response = self.send_command(Command::SetBand(band))?;
        match response {
            Response::Ok => Ok(()),
            r => Err(DriverError::UnexpectedResponse(r)),
        }
    }

    /// Set the mode of operation, peer to peer or network mode.
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

    /// Transmit data using the specified confirmation mode and given port.
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

    /// Poll for any received data and copy it to the provided buffer. If data have been received,
    /// the length of the data is returned.
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

    /// Attempt to read data from UART and store it in the parse buffer. This should
    /// be invoked whenever data should be read.
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

    /// Attempt to parse the internal buffer and enqueue any response data found.
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

    // Block until a response is received.
    fn recv_response(&mut self) -> Result<Response, DriverError> {
        loop {
            // Run processing to increase likelyhood we have something to parse.
            for _ in 0..1000 {
                self.process()?;
            }
            self.digest()?;
            if let Some(response) = self.rxq.dequeue() {
                return Ok(response);
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

    /// Send an AT command to the lora module and await a response.
    pub fn send_command(&mut self, command: Command) -> Result<Response, DriverError> {
        let mut s = Command::buffer();
        command.encode(&mut s);
        log::debug!("Sending command {}", s.as_str());
        self.do_write(s.as_bytes())?;
        self.do_write(b"\r\n")?;

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
