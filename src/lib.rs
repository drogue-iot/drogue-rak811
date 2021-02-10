#![no_std]
//!A network driver for a RAK811 attached via a UART.
//!
//!Currently requires the RAK811 to be flashed with a 2.x version of the AT firmware.
//!
//!At first, the UART must be configured and handed to the driver. The uart must implement the `embedded_hal::serial` traits.
//!
//!## Usage
//!
//!```rust
//!let (uarte_tx, uarte_rx) = uarte
//!    .split(ctx.resources.tx_buf, ctx.resources.rx_buf)
//!    .unwrap();
//!
//!
//!let driver = rak811::Rak811Driver::new(
//!    uarte_tx,
//!    uarte_rx,
//!    port1.p1_02.into_push_pull_output(Level::High).degrade(),
//!)
//!.unwrap();
//!```
//!
//!In order to connect to the gateway, the LoRa node needs to be configured with the following:
//!
//!* Frequency band - This depends on where you live.
//!* Mode of operation - This can either be LoRa P2P which allows the node to send and receive data directly from another LoRa node, or LoRaWAN which connects the node to a gateway.
//!
//!The driver can be used to configure the properties in this way:
//!
//!```rust
//!driver.set_band(rak811::LoraRegion::EU868).unwrap();
//!driver.set_mode(rak811::LoraMode::WAN).unwrap();
//!```
//!
//!In addition, the following settings from the TTN console must be set:
//!
//!* Device EUI
//!* Application EUI
//!* Application Key
//!
//!```rust
//!driver.set_device_eui(&[0x00, 0xBB, 0x7C, 0x95, 0xAD, 0xB5, 0x30, 0xB9]).unwrap();
//!driver.set_app_eui(&[0x70, 0xB3, 0xD5, 0x7E, 0xD0, 0x03, 0xB1, 0x84])
//!
//!// Secret generated by network provider
//!driver .set_app_key(&[0x00]).unwrap();
//!```
//!
//!To join the network and send packets:
//!
//!```rust
//!driver.join(lora::ConnectMode::OTAA).unwrap();
//!
//!// Port number can be between 1 and 255
//!driver.send(lora::QoS::Confirmed, 1, b"hello!").unwrap();
//!```

use embedded_hal::digital::v2::OutputPin;
use embedded_hal::{serial::Read, serial::Write};
mod buffer;
mod error;
mod parser;
mod protocol;

pub use buffer::*;
pub use drogue_lora::*;
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
            Response::Initialized(band) => {
                self.lora_band = band;
                Ok(())
            }
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
                    Response::Initialized(band) => {
                        self.lora_band = band;
                        Ok(())
                    }
                    _ => Err(DriverError::NotInitialized),
                }
            }
            r => log_unexpected(r),
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
                    r => log_unexpected(r),
                }
            }
            r => log_unexpected(r),
        }
    }

    /// Set the frequency band based on the region.
    pub fn set_band(&mut self, band: LoraRegion) -> Result<(), DriverError> {
        if self.lora_band != band {
            self.lora_band = band;
            let response = self.send_command(Command::SetBand(band))?;
            match response {
                Response::Ok => Ok(()),
                r => log_unexpected(r),
            }
        } else {
            Ok(())
        }
    }

    /// Set the mode of operation, peer to peer or network mode.
    pub fn set_mode(&mut self, mode: LoraMode) -> Result<(), DriverError> {
        self.lora_mode = mode;
        let response = self.send_command(Command::SetMode(mode))?;
        match response {
            Response::Ok => Ok(()),
            r => log_unexpected(r),
        }
    }

    pub fn set_device_address(&mut self, addr: &DevAddr) -> Result<(), DriverError> {
        let response = self.send_command(Command::SetConfig(ConfigOption::DevAddr(addr)))?;
        match response {
            Response::Ok => Ok(()),
            r => log_unexpected(r),
        }
    }

    pub fn set_device_eui(&mut self, eui: &EUI) -> Result<(), DriverError> {
        let response = self.send_command(Command::SetConfig(ConfigOption::DevEui(eui)))?;
        match response {
            Response::Ok => Ok(()),
            r => log_unexpected(r),
        }
    }
    pub fn set_app_eui(&mut self, eui: &EUI) -> Result<(), DriverError> {
        let response = self.send_command(Command::SetConfig(ConfigOption::AppEui(eui)))?;
        match response {
            Response::Ok => Ok(()),
            r => log_unexpected(r),
        }
    }

    pub fn set_app_key(&mut self, key: &AppKey) -> Result<(), DriverError> {
        let response = self.send_command(Command::SetConfig(ConfigOption::AppKey(key)))?;
        match response {
            Response::Ok => Ok(()),
            r => log_unexpected(r),
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
                    r => log_unexpected(r),
                }
            }
            r => log_unexpected(r),
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

fn log_unexpected(r: Response) -> Result<(), DriverError> {
    log::error!("Unexpected response: {:?}", r);
    Err(DriverError::UnexpectedResponse)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
