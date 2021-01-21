use crate::gpiote::*;
use drogue_device::prelude::*;
use hal::gpio::{Input, Pin, PullUp};
use hal::pac::Interrupt;
use nrf52833_hal as hal;

pub type Button = GpioteChannel<Pin<Input<PullUp>>>;

pub struct LoraDevice {
    pub gpiote: InterruptContext<Gpiote>,
    pub btn_fwd: ActorContext<Button>,
    pub btn_back: ActorContext<Button>,
    pub gpiote_to_fwd: GpioteToChannel,
    pub gpiote_to_back: GpioteToChannel,
}

impl Device for LoraDevice {
    fn start(&'static mut self, supervisor: &mut Supervisor) {
        let gpiote_addr = self.gpiote.start(supervisor);
        let fwd_addr = self.btn_fwd.start(supervisor);
        let back_addr = self.btn_back.start(supervisor);

        self.gpiote_to_fwd.set_address(fwd_addr);
        self.gpiote_to_back.set_address(back_addr);

        log::info!("Address set, notifying..");
        gpiote_addr.notify(AddSink::new(&self.gpiote_to_fwd));
        gpiote_addr.notify(AddSink::new(&self.gpiote_to_back));
        log::info!("Notified...");
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
