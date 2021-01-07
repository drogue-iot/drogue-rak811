use nom::alt;
use nom::char;
use nom::character::streaming::digit1;
use nom::do_parse;
use nom::named;
use nom::opt;
use nom::tag;
use nom::tuple;
use nom::IResult;

use super::{FirmwareInfo, LoraRegion, Response};

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

fn parse_u8(input: &[u8]) -> IResult<&[u8], u8> {
    let (input, digits) = digit1(input)?;
    IResult::Ok((input, atoi_u8(digits).unwrap()))
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

#[rustfmt::skip]
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

#[rustfmt::skip]
named!(
    pub lora_region<LoraRegion>,
    do_parse!(
        region: alt!(
            tag!("EU868") |
            tag!("US915") |
            tag!("AU915") |
            tag!("KR920") |
            tag!("AS923") |
            tag!("IN865")
        ) >>
        (
                LoraRegion::parse(region)
        )
    )
);

#[rustfmt::skip]
named!(
    pub lora_band<Response>,
    do_parse!(
        tag!("OK") >>
        region: lora_region >>
        crlf >>
        (
            Response::LoraBand(region)
        )
    )
);

named!(
    pub parse<Response>,
    alt!(
          ok
        | error
        | firmware_info
        | lora_band
    )
);

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
