use core::fmt::Write;
use heapless::{consts, String};

#[derive(Debug)]
pub enum Command {
    QueryFirmwareInfo,
    SetBand(LoraRegion),
    SetMode(LoraMode),
    GetBand,
    Join(ConnectMode),
    SetConfig(ConfigOption),
    GetConfig(ConfigKey),
    Reset(ResetMode),
}

#[derive(Debug)]
pub enum ResetMode {
    Restart,
    Reload,
}

#[derive(Debug)]
pub enum ConnectMode {
    OTAA,
    ABP,
}

#[derive(Debug)]
#[repr(u8)]
pub enum LoraMode {
    WAN = 0,
    P2P = 1,
}

#[derive(Debug)]
pub struct DevAddr([u8; 4]);

#[derive(Debug)]
pub struct EUI([u8; 8]);

#[derive(Debug)]
pub struct AppKey([u8; 16]);

#[derive(Debug)]
pub struct NwksKey([u8; 16]);

#[derive(Debug)]
pub struct AppsKey([u8; 16]);

#[derive(Debug)]
pub enum ConfigKey {
    DevAddr,
    DevEui,
    AppEui,
    AppKey,
    NwksKey,
    AppsKey,
}

#[derive(Debug)]
pub enum ConfigOption {
    DevAddr(DevAddr),
    DevEui(EUI),
    AppEui(EUI),
    AppKey(AppKey),
    NwksKey(NwksKey),
    AppsKey(AppsKey),
    /*
    PwrLevel,
    Adr,
    Dr,
    PublicNet,
    RxDelay1,
    Rx2,
    ChList,
    ChMask,
    MaxChs,
    JoinCnt,
    Nbtrans,
    Class,
    Duty,*/
}

#[derive(Debug)]
pub enum Response {
    None,
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

pub type CommandBuffer = String<consts::U128>;

impl Command {
    pub fn buffer() -> CommandBuffer {
        String::new()
    }

    pub fn encode(&self, s: &mut CommandBuffer) {
        match self {
            Command::QueryFirmwareInfo => {
                write!(s, "at+version").unwrap();
            }
            Command::SetBand(region) => {
                write!(s, "at+band=").unwrap();
                region.encode(s);
            }
            Command::GetBand => {
                write!(s, "at+band").unwrap();
            }
            Command::SetMode(mode) => {
                write!(s, "at+mode=").unwrap();
                mode.encode(s);
            }
            Command::Join(mode) => {
                write!(s, "at+join=").unwrap();
                mode.encode(s);
            }
            Command::SetConfig(opt) => {
                write!(s, "at+set_config=").unwrap();
                opt.encode(s);
            }
            Command::GetConfig(key) => {
                write!(s, "at+get_config=").unwrap();
                key.encode(s);
            }
            Command::Reset(mode) => {
                write!(
                    s,
                    "at+reset={}",
                    match mode {
                        ResetMode::Restart => 0,
                        ResetMode::Reload => 1,
                    }
                )
                .unwrap();
            }
        }
    }
}

impl ConfigKey {
    pub fn encode(&self, s: &mut CommandBuffer) {
        match self {
            ConfigKey::DevAddr => {
                s.push_str("dev_addr");
            }
            ConfigKey::DevEui => {
                s.push_str("dev_eui");
            }
            ConfigKey::AppEui => {
                s.push_str("app_eui");
            }
            ConfigKey::AppKey => {
                s.push_str("app_key");
            }
            ConfigKey::NwksKey => {
                s.push_str("nwks_key");
            }
            ConfigKey::AppsKey => {
                s.push_str("apps_key");
            }
        }
    }
}

impl ConfigOption {
    pub fn encode(&self, s: &mut CommandBuffer) {
        match self {
            ConfigOption::DevAddr(addr) => {
                write!(
                    s,
                    "dev_addr:{}{}{}{}",
                    addr.0[0], addr.0[1], addr.0[2], addr.0[3]
                )
                .unwrap();
            }
            ConfigOption::DevEui(eui) => {
                write!(
                    s,
                    "dev_eui:{}{}{}{}{}{}{}{}",
                    eui.0[0], eui.0[1], eui.0[2], eui.0[3], eui.0[4], eui.0[5], eui.0[6], eui.0[7]
                )
                .unwrap();
            }
            ConfigOption::AppEui(eui) => {
                write!(
                    s,
                    "app_eui:{}{}{}{}{}{}{}{}",
                    eui.0[0], eui.0[1], eui.0[2], eui.0[3], eui.0[4], eui.0[5], eui.0[6], eui.0[7]
                )
                .unwrap();
            }
            ConfigOption::AppKey(key) => {
                write!(
                    s,
                    "app_key:{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
                    key.0[0],
                    key.0[1],
                    key.0[2],
                    key.0[3],
                    key.0[4],
                    key.0[5],
                    key.0[6],
                    key.0[7],
                    key.0[8],
                    key.0[9],
                    key.0[10],
                    key.0[11],
                    key.0[12],
                    key.0[13],
                    key.0[14],
                    key.0[15]
                )
                .unwrap();
            }
            ConfigOption::NwksKey(key) => {
                write!(
                    s,
                    "nwks_key:{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
                    key.0[0],
                    key.0[1],
                    key.0[2],
                    key.0[3],
                    key.0[4],
                    key.0[5],
                    key.0[6],
                    key.0[7],
                    key.0[8],
                    key.0[9],
                    key.0[10],
                    key.0[11],
                    key.0[12],
                    key.0[13],
                    key.0[14],
                    key.0[15]
                )
                .unwrap();
            }
            ConfigOption::AppsKey(key) => {
                write!(
                    s,
                    "apps_key:{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
                    key.0[0],
                    key.0[1],
                    key.0[2],
                    key.0[3],
                    key.0[4],
                    key.0[5],
                    key.0[6],
                    key.0[7],
                    key.0[8],
                    key.0[9],
                    key.0[10],
                    key.0[11],
                    key.0[12],
                    key.0[13],
                    key.0[14],
                    key.0[15]
                )
                .unwrap();
            }
        }
    }
}

impl ConnectMode {
    pub fn encode(&self, s: &mut CommandBuffer) {
        let val = match self {
            ConnectMode::OTAA => "otaa",
            ConnectMode::ABP => "abp",
        };
        s.push_str(val).unwrap();
    }

    pub fn parse(d: &[u8]) -> ConnectMode {
        if let Ok(s) = core::str::from_utf8(d) {
            match s {
                "abp" => ConnectMode::ABP,
                _ => ConnectMode::OTAA,
            }
        } else {
            ConnectMode::OTAA
        }
    }
}

impl LoraMode {
    pub fn encode(&self, s: &mut CommandBuffer) {
        let val = match self {
            LoraMode::WAN => "0",
            LoraMode::P2P => "1",
        };
        s.push_str(val).unwrap();
    }

    pub fn parse(d: &[u8]) -> LoraMode {
        if let Ok(s) = core::str::from_utf8(d) {
            match s {
                "1" => LoraMode::P2P,
                _ => LoraMode::WAN,
            }
        } else {
            LoraMode::WAN
        }
    }
}

impl LoraRegion {
    pub fn encode(&self, s: &mut CommandBuffer) {
        let val = match self {
            LoraRegion::EU868 => "EU868",
            LoraRegion::US915 => "US915",
            LoraRegion::AU915 => "AU915",
            LoraRegion::KR920 => "KR920",
            LoraRegion::AS923 => "AS923",
            LoraRegion::IN865 => "IN865",
            LoraRegion::UNKNOWN => "UNKNOWN",
        };
        s.push_str(val).unwrap();
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
