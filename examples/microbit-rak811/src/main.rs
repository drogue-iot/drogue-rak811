//! Example showing the use of a LoRa breakout board using the RAK811 network driver.
#![no_main]
#![no_std]

use panic_halt as _;

use core::sync::atomic::{compiler_fence, Ordering};
use embedded_hal::digital::v2::OutputPin;
use log::LevelFilter;
use rtic::app;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use nrf52833_hal as hal;

use drogue_rak811 as rak811;
use hal::gpio::{Level, Output, Pin, PushPull};
use hal::pac::UARTE0;
use hal::uarte::*;

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);

#[app(device = crate::hal::pac, peripherals = true)]
const APP: () = {
    struct Resources {
        #[init([0; 1])]
        rx_buf: [u8; 1],
        #[init([0; 128])]
        tx_buf: [u8; 128],
        driver: rak811::Rak811Driver<UarteTx<UARTE0>, UarteRx<UARTE0>, Pin<Output<PushPull>>>,
    }

    #[init(resources = [tx_buf, rx_buf])]
    fn init(ctx: init::Context) -> init::LateResources {
        rtt_init_print!();
        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(log::LevelFilter::Info);

        let port0 = hal::gpio::p0::Parts::new(ctx.device.P0);
        let port1 = hal::gpio::p1::Parts::new(ctx.device.P1);

        let clocks = hal::clocks::Clocks::new(ctx.device.CLOCK).enable_ext_hfosc();
        let _clocks = clocks.start_lfclk();

        let uarte = Uarte::new(
            ctx.device.UARTE0,
            Pins {
                txd: port0.p0_01.into_push_pull_output(Level::High).degrade(),
                rxd: port0.p0_13.into_floating_input().degrade(),
                cts: None,
                rts: None,
            },
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        );

        let (uarte_tx, uarte_rx) = uarte
            .split(ctx.resources.tx_buf, ctx.resources.rx_buf)
            .unwrap();

        let driver = rak811::Rak811Driver::new(
            uarte_tx,
            uarte_rx,
            port1.p1_02.into_push_pull_output(Level::High).degrade(),
        )
        .unwrap();

        log::info!("Driver initialized");

        init::LateResources { driver }
    }

    #[idle(resources=[driver])]
    fn idle(ctx: idle::Context) -> ! {
        let idle::Resources { driver } = ctx.resources;

        log::info!("Configuring driver");
        driver
            .set_band(rak811::LoraRegion::EU868)
            .map_err(|e| log::error!("ERROR: {:?}", e))
            .unwrap();

        driver
            .set_mode(rak811::LoraMode::WAN)
            .map_err(|e| log::error!("ERROR: {:?}", e))
            .unwrap();

        // TODO: Set the following settings to values provided by your network.
        driver
            .set_device_eui(&[0x00, 0xBB, 0x7C, 0x95, 0xAD, 0xB5, 0x30, 0xB9])
            .map_err(|e| log::error!("ERROR: {:?}", e))
            .unwrap();

        driver
            .set_app_eui(&[0x70, 0xB3, 0xD5, 0x7E, 0xD0, 0x03, 0xB1, 0x84])
            .map_err(|e| log::error!("ERROR: {:?}", e))
            .unwrap();

        driver
            .set_app_key(&[
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ])
            .map_err(|e| log::error!("ERROR: {:?}", e))
            .unwrap();

        // Join using OTAA
        log::info!("Joining network");
        driver
            .join(rak811::ConnectMode::OTAA)
            .map_err(|e| log::error!("ERROR: {:?}", e))
            .unwrap();

        log::info!("Sending data");
        driver
            .send(rak811::QoS::Confirmed, 1, b"hello")
            .map_err(|e| log::error!("ERROR: {:?}", e))
            .unwrap();

        log::info!("Data sent!");
        loop {
            cortex_m::asm::nop();
        }
    }
};
