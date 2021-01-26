#![deny(unsafe_code)]
#![no_main]
#![no_std]

extern crate cortex_m;
extern crate cortex_m_rt as rt;
extern crate panic_halt;
extern crate stm32l1xx_hal as hal;
// extern crate sx127x_lora;

use core::sync::atomic::{compiler_fence, Ordering};
use embedded_hal::digital::v2::OutputPin;
use embedded_hal::digital::v2::ToggleableOutputPin;
use log::LevelFilter;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use hal::delay::Delay;
use hal::gpio::GpioExt;
use hal::prelude::*;
use hal::rcc::Config;
use hal::rcc::RccExt;
use hal::spi::*;
use hal::time::*;
use hal::{spi, stm32};
use rt::entry;
//use sx127x_lora::MODE;

const FREQUENCY: i64 = 868;
static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Trace);

#[entry]
fn main() -> ! {
    rtt_init_print!();
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    log::info!("Init");
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let mut rcc = dp.RCC.freeze(Config::hsi());
    let mut delay = cp.SYST.delay(rcc.clocks);

    let gpioa = dp.GPIOA.split();
    let gpiob = dp.GPIOB.split();
    let mut led = gpioa.pa12.into_push_pull_output();

    let sck = gpioa.pa5;
    let miso = gpioa.pa6;
    let mosi = gpioa.pa7;
    let reset = gpiob.pb13.into_push_pull_output();
    let cs = gpiob.pb0.into_push_pull_output();

    /*
        let mut spi = dp
            .SPI1
            .spi((sck, miso, mosi), spi::MODE_0, 100.khz(), &mut rcc);

        let r = {
            /*
            let mut lora = sx127x_lora::LoRa::new(spi, cs, reset, FREQUENCY, delay);*/
            match lora {
                Ok(_) => true,
                Err(_) => false,
            }
        };
    */

    loop {
        if true {
            blink(200, 500, &mut led, &mut delay);
        } else {
            blink(2, 2000, &mut led, &mut delay);
        }
        compiler_fence(Ordering::SeqCst);
    }
    /*
        let poll = lora.poll_irq(Some(30)); //30 Second timeout
        match poll {
            Ok(size) => {
                let buffer = lora.read_packet(); // Received buffer. NOTE: 255 bytes are always returned

                //blink(2, 200, &mut led);
                led.set_low().ok().unwrap();
            }
            Err(_) => {
                led.set_high().ok().unwrap();
                //                blink(2, 2000, &mut led);
            }
        }
    }*/
}

fn blink<LED: OutputPin + ToggleableOutputPin>(
    ntimes: u32,
    period: u32,
    led: &mut LED,
    delay: &mut Delay,
) {
    led.set_high().ok().unwrap();
    for i in 0..ntimes {
        led.toggle().ok().unwrap();
        delay.delay(period.ms());
    }
    led.set_high().ok().unwrap();
}
