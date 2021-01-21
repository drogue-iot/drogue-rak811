use crate::gpiote::*;
use drogue_device::prelude::*;
use hal::gpio::{Input, Pin, PullUp};
use hal::pac::Interrupt;
use nrf52833_hal as hal;

pub type Button = GpioteChannel<Pin<Input<PullUp>>>;

pub struct LoraDevice {
    pub gpiote: InterruptContext<Gpiote>,
    pub button: ActorContext<Button>,
    pub gpiote_to_button: GpioteToChannel,
}

impl Device for LoraDevice {
    fn start(&'static mut self, supervisor: &mut Supervisor) {
        let gpiote_addr = self.gpiote.start(supervisor);
        let button_addr = self.button.start(supervisor);
        log::info!("Start invoked");
        self.gpiote_to_button.set_address(button_addr);
        gpiote_addr.notify(AddSink {
            sink: &self.gpiote_to_button,
        });
    }
}

pub struct GpioteToChannel {
    sink: Option<Address<Button>>,
}

impl GpioteToChannel {
    pub fn new() -> Self {
        Self { sink: None }
    }
    pub fn set_address(&mut self, address: Address<Button>) {
        self.sink.replace(address);
    }
}

impl Sink<GpioteEvent> for GpioteToChannel {
    fn notify(&self, event: GpioteEvent) {
        self.sink.as_ref().map(|sink| sink.notify(event));
    }
}
