#![no_std]

use embedded_hal::{digital::v2::OutputPin, serial::Read, serial::Write};
use nb;

pub struct Rak811Driver<W, R>
where
    W: Write<u8>,
    R: Read<u8>,
{
    tx: W,
    rx: R,
}

pub trait Timer {}

impl<W, R> Rak811Driver<W, R>
where
    W: Write<u8>,
    R: Read<u8>,
{
    pub fn new(tx: W, rx: R) -> Rak811Driver<W, R> {
        Rak811Driver { tx, rx }
    }

    pub fn send(&mut self, command: Command) -> Result<Response, AdapterError> {
        command.write(&mut self.tx)?;

        let response = self.recv()?;
        Ok(response)
    }

    pub fn recv(&mut self) -> Result<Response, AdapterError> {
        let mut res = [0; 64];
        let mut rp = 0;
        loop {
            let b = nb::block!(self.rx.read()).map_err(|_| AdapterError::ReadError)?;
            res[rp] = b;
            rp += 1;
            if b as char == '\n' {
                break;
            }
        }

        let mut ret = Err(AdapterError::ReadError);
        if rp > 0 {
            if let Ok(msg) = core::str::from_utf8(&res[..rp]) {
                log::info!("Res: {}", msg);
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

#[derive(Debug)]
pub enum Command {
    QueryFirmwareInfo,
}

fn write_str<W: Write<u8>>(w: &mut W, s: &str) -> Result<(), AdapterError> {
    for &b in s.as_bytes() {
        nb::block!(w.write(b)).map_err(|_| AdapterError::WriteError)?;
    }
    Ok(())
}

impl Command {
    pub fn write<W: Write<u8>>(&self, w: &mut W) -> Result<(), AdapterError> {
        match self {
            Command::QueryFirmwareInfo => {
                write_str(w, "at+version\r\n")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Response {
    Ok,
    Error(i8),
    FirmwareInfo(FirmwareInfo),
}

/// Version information for the ESP board.
#[derive(Debug)]
pub struct FirmwareInfo {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub build: u8,
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

#[derive(Debug)]
pub enum AdapterError {
    WriteError,
    ReadError,
}

impl core::convert::From<core::fmt::Error> for AdapterError {
    fn from(_error: core::fmt::Error) -> Self {
        AdapterError::WriteError
    }
}

mod parser {

    use nom::alt;
    use nom::char;
    use nom::character::streaming::digit1;
    use nom::do_parse;
    use nom::named;
    use nom::opt;
    use nom::tag;
    use nom::take;
    use nom::take_until;
    use nom::tuple;
    use nom::IResult;

    use super::{FirmwareInfo, Response};

    fn ascii_to_digit(character: u8) -> Option<u8> {
        match character {
            b'0' => Some(0),
            b'1' => Some(1),
            b'2' => Some(2),
            b'3' => Some(3),
            b'4' => Some(4),
            b'5' => Some(5),
            b'6' => Some(6),
            b'7' => Some(7),
            b'8' => Some(8),
            b'9' => Some(9),
            _ => None,
        }
    }

    fn atoi_u8(digits: &[u8]) -> Option<u8> {
        let mut num: u8 = 0;
        let len = digits.len();
        for (i, digit) in digits.iter().enumerate() {
            let digit = ascii_to_digit(*digit)?;
            let mut exp = 1;
            for _ in 0..(len - i - 1) {
                exp *= 10;
            }
            num += exp * digit;
        }
        Some(num)
    }

    fn atoi_usize(digits: &[u8]) -> Option<usize> {
        let mut num: usize = 0;
        let len = digits.len();
        for (i, digit) in digits.iter().enumerate() {
            let digit = ascii_to_digit(*digit)? as usize;
            let mut exp = 1;
            for _ in 0..(len - i - 1) {
                exp *= 10;
            }
            num += exp * digit;
        }
        Some(num)
    }

    fn parse_u8(input: &[u8]) -> IResult<&[u8], u8> {
        let (input, digits) = digit1(input)?;
        IResult::Ok((input, atoi_u8(digits).unwrap()))
    }

    fn parse_usize(input: &[u8]) -> IResult<&[u8], usize> {
        let (input, digits) = digit1(input)?;
        let num = atoi_usize(digits).unwrap();
        IResult::Ok((input, num))
    }

    #[rustfmt::skip]
named!(
    crlf,
    tag!("\r\n")
);

    #[rustfmt::skip]
named!(
    pub ok<Response>,
    do_parse!(
        tuple!(
            opt!(crlf),
            opt!(crlf),
            tag!("OK"),
            crlf
        ) >>
        (
            Response::Ok
        )
    )
);

    named!(
        pub error<Response>,
        do_parse!(
            opt!(crlf) >>
            opt!(crlf) >>
            tag!("ERROR") >>
            sign: opt!(char!('-')) >>
            code: parse_u8 >>
            crlf >>
            (
                Response::Error(sign.map(|s| if s == '-' { - (code as i8) } else {code as i8}).unwrap_or(code as i8))
            )
        )
    );

    #[rustfmt::skip]
named!(
    pub firmware_info<Response>,
    do_parse!(
        tag!("OK") >>
        major: parse_u8 >>
        tag!(".") >>
        minor: parse_u8 >>
        tag!(".") >>
        patch: parse_u8 >>
        tag!(".") >>
        build: parse_u8 >>
        crlf >>
        (
            Response::FirmwareInfo(FirmwareInfo{major, minor, patch, build})
        )
    )
);

    named!(
        pub parse<Response>,
        alt!(
              ok
            | error
            | firmware_info
        )
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
