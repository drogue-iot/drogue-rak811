use crate::error::*;
use embedded_hal::{serial::Read, serial::Write};

#[derive(Debug)]
pub enum Command {
    QueryFirmwareInfo,
    SetBand(LoraRegion),
    GetBand,
}

#[derive(Debug)]
pub enum Response {
    Ok,
    Error(i8),
    FirmwareInfo(FirmwareInfo),
    LoraBand(LoraRegion),
}

/// Version information for the RAK811 board
#[derive(Debug)]
pub struct FirmwareInfo {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub build: u8,
}

#[derive(Debug)]
pub enum LoraRegion {
    EU868,
    US915,
    AU915,
    KR920,
    AS923,
    IN865,
    UNKNOWN,
}

impl Command {
    pub fn write<W: Write<u8>>(&self, w: &mut W) -> Result<(), DriverError> {
        match self {
            Command::QueryFirmwareInfo => {
                write_str(w, "at+version")?;
            }
            Command::SetBand(region) => {
                write_str(w, "at+band=")?;
                write_str(w, region.as_str())?;
            }
            Command::GetBand => {
                write_str(w, "at+band")?;
            }
        }
        write_str(w, "\r\n")?;
        Ok(())
    }
}

impl LoraRegion {
    fn as_str(&self) -> &str {
        match self {
            LoraRegion::EU868 => "EU868",
            LoraRegion::US915 => "US915",
            LoraRegion::AU915 => "AU915",
            LoraRegion::KR920 => "KR920",
            LoraRegion::AS923 => "AS923",
            LoraRegion::IN865 => "IN865",
            LoraRegion::UNKNOWN => "UNKNOWN",
        }
    }

    pub fn parse(d: &[u8]) -> LoraRegion {
        if let Ok(s) = core::str::from_utf8(d) {
            match s {
                "EU868" => LoraRegion::EU868,
                "US915" => LoraRegion::US915,
                "AU915" => LoraRegion::AU915,
                "KR920" => LoraRegion::KR920,
                "AS923" => LoraRegion::AS923,
                "IN865" => LoraRegion::IN865,
                _ => LoraRegion::UNKNOWN,
            }
        } else {
            LoraRegion::UNKNOWN
        }
    }
}

fn write_str<W: Write<u8>>(w: &mut W, s: &str) -> Result<(), DriverError> {
    for &b in s.as_bytes() {
        nb::block!(w.write(b)).map_err(|_| DriverError::WriteError)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
