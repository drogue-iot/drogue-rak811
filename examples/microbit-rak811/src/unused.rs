

    /*
    #[task(binds = UARTE0_UART0, resources = [uarte_tx, uarte_rx])]
    fn uarte0(ctx: uarte0::Context) {
        let uarte0::Resources { uarte_tx, uarte_rx } = ctx.resources;

        uarte_tx.process_interrupt();
        let r = uarte_rx.process_interrupt();
        match r {
            Ok(_) => log::info!("rx process interrupt OK"),
            Err(_) => log::info!("rx process interrupt ERR"),
        }

        /*
        if uarte.read_done() {
            uarte.finalize_read();
            let read = uarte.num_read() as usize;
            if let Ok(msg) = core::str::from_utf8(&rx_buffer[..]) {
                log::info!("R: {}", msg);
            }
            log::info!("Buffer conents: {:?}", rx_buffer);
        }

        if uarte.ready() {
            uarte.clear_ready();
        }*/
    }*/
};

/*
mod serial {

    use core::fmt;
    use core::ops::Deref;
    use core::sync::atomic::{compiler_fence, Ordering::SeqCst};
    use embedded_hal::digital::v2::OutputPin;
    use embedded_hal::serial;
    use hal::gpio::{Floating, Input, Output, Pin, PushPull};
    use hal::pac::{uarte0, UARTE0};
    use nb;
    use nrf52833_hal as hal;

    use target_constants::EASY_DMA_SIZE;
    pub use uarte0::{baudrate::BAUDRATE_A as Baudrate, config::PARITY_A as Parity};

    pub struct Uarte<T>(T);

    #[derive(Debug)]
    pub struct DMABuffer<'a> {
        buffer: &'a mut [u8],
        written: usize,
    }

    impl<'a> DMABuffer<'a> {
        pub fn new(buffer: &'a mut [u8]) -> DMABuffer {
            DMABuffer { buffer, written: 0 }
        }

        fn write(&mut self, data: &[u8]) -> Result<(), Error> {
            if data.len() > self.buffer.len() - self.written {
                return Err(Error::TxBufferTooLong);
            }

            self.buffer[self.written] = data[0];
            self.written += data.len();
            Ok(())
        }

        fn clear(&mut self) {
            self.written = 0;
        }

        fn size(&self) -> usize {
            self.buffer.len()
        }

        fn set_len(&mut self, len: usize) {
            self.written = len;
        }

        fn len(&self) -> usize {
            self.written
        }

        fn read(&self, pos: usize) -> Result<u8, Error> {
            Ok(self.buffer[pos])
        }
    }

    impl<T> Uarte<T>
    where
        T: Instance,
    {
        pub fn split<'a>(
            self,
            txp: DMABuffer<'a>,
            rxp: DMABuffer<'a>,
        ) -> (UarteTx<'a, T>, UarteRx<'a, T>) {
            let tx = UarteTx::new(txp);
            let rx = UarteRx::new(rxp);
            (tx, rx)
        }

        pub fn new(uarte: T, mut pins: Pins, parity: Parity, baudrate: Baudrate) -> Self {
            // Select pins
            uarte.psel.rxd.write(|w| {
                let w = unsafe { w.pin().bits(pins.rxd.pin()) };
                #[cfg(any(feature = "52833", feature = "52840"))]
                let w = w.port().bit(pins.rxd.port().bit());
                w.connect().connected()
            });
            pins.txd.set_high().unwrap();
            uarte.psel.txd.write(|w| {
                let w = unsafe { w.pin().bits(pins.txd.pin()) };
                #[cfg(any(feature = "52833", feature = "52840"))]
                let w = w.port().bit(pins.txd.port().bit());
                w.connect().connected()
            });

            // Optional pins
            uarte.psel.cts.write(|w| {
                if let Some(ref pin) = pins.cts {
                    let w = unsafe { w.pin().bits(pin.pin()) };
                    #[cfg(any(feature = "52833", feature = "52840"))]
                    let w = w.port().bit(pin.port().bit());
                    w.connect().connected()
                } else {
                    w.connect().disconnected()
                }
            });

            uarte.psel.rts.write(|w| {
                if let Some(ref pin) = pins.rts {
                    let w = unsafe { w.pin().bits(pin.pin()) };
                    #[cfg(any(feature = "52833", feature = "52840"))]
                    let w = w.port().bit(pin.port().bit());
                    w.connect().connected()
                } else {
                    w.connect().disconnected()
                }
            });

            // Enable UARTE instance.
            uarte.enable.write(|w| w.enable().enabled());

            // Configure.
            let hardware_flow_control = pins.rts.is_some() && pins.cts.is_some();
            uarte
                .config
                .write(|w| w.hwfc().bit(hardware_flow_control).parity().variant(parity));

            // Configure frequency.
            uarte.baudrate.write(|w| w.baudrate().variant(baudrate));

            Uarte(uarte)
        }

        /// Write via UARTE.
        ///
        /// This method uses transmits all bytes in `tx_buffer`.
        ///
        /// The buffer must have a length of at most 255 bytes on the nRF52832
        /// and at most 65535 bytes on the nRF52840.
        pub fn write(&mut self, tx_buffer: &[u8]) -> Result<(), Error> {
            if tx_buffer.len() > EASY_DMA_SIZE {
                return Err(Error::TxBufferTooLong);
            }

            // We can only DMA out of RAM.
            slice_in_ram_or(tx_buffer, Error::BufferNotInRAM)?;

            // Conservative compiler fence to prevent optimizations that do not
            // take in to account actions by DMA. The fence has been placed here,
            // before any DMA action has started.
            compiler_fence(SeqCst);

            // Reset the events.
            self.0.events_endtx.reset();
            self.0.events_txstopped.reset();

            // Set up the DMA write.
            self.0.txd.ptr.write(|w|
            // We're giving the register a pointer to the stack. Since we're
            // waiting for the UARTE transaction to end before this stack pointer
            // becomes invalid, there's nothing wrong here.
            //
            // The PTR field is a full 32 bits wide and accepts the full range
            // of values.
            unsafe { w.ptr().bits(tx_buffer.as_ptr() as u32) });
            self.0.txd.maxcnt.write(|w|
            // We're giving it the length of the buffer, so no danger of
            // accessing invalid memory. We have verified that the length of the
            // buffer fits in an `u8`, so the cast to `u8` is also fine.
            //
            // The MAXCNT field is 8 bits wide and accepts the full range of
            // values.
            unsafe { w.maxcnt().bits(tx_buffer.len() as _) });

            // Start UARTE Transmit transaction.
            self.0.tasks_starttx.write(|w|
            // `1` is a valid value to write to task registers.
            unsafe { w.bits(1) });

            // Wait for transmission to end.
            let mut endtx;
            let mut txstopped;
            loop {
                endtx = self.0.events_endtx.read().bits() != 0;
                txstopped = self.0.events_txstopped.read().bits() != 0;
                if endtx || txstopped {
                    break;
                }
            }

            // Conservative compiler fence to prevent optimizations that do not
            // take in to account actions by DMA. The fence has been placed here,
            // after all possible DMA actions have completed.
            compiler_fence(SeqCst);

            if txstopped {
                return Err(Error::Transmit);
            }

            // Lower power consumption by disabling the transmitter once we're
            // finished.
            self.0.tasks_stoptx.write(|w|
            // `1` is a valid value to write to task registers.
            unsafe { w.bits(1) });

            Ok(())
        }

        /// Read via UARTE.
        ///
        /// This method fills all bytes in `rx_buffer`, and blocks
        /// until the buffer is full.
        ///
        /// The buffer must have a length of at most 255 bytes.
        pub fn read(&mut self, rx_buffer: &mut [u8]) -> Result<(), Error> {
            self.start_read(rx_buffer)?;

            // Wait for transmission to end.
            while self.0.events_endrx.read().bits() == 0 {}

            self.finalize_read();

            if self.0.rxd.amount.read().bits() != rx_buffer.len() as u32 {
                return Err(Error::Receive);
            }

            Ok(())
        }

        /// Start a UARTE read transaction by setting the control
        /// values and triggering a read task.
        fn start_read(&mut self, rx_buffer: &mut [u8]) -> Result<(), Error> {
            // This is overly restrictive. See (similar SPIM issue):
            // https://github.com/nrf-rs/nrf52/issues/17
            if rx_buffer.len() > u8::max_value() as usize {
                return Err(Error::RxBufferTooLong);
            }

            // NOTE: RAM slice check is not necessary, as a mutable slice can only be
            // built from data located in RAM.

            // Conservative compiler fence to prevent optimizations that do not
            // take in to account actions by DMA. The fence has been placed here,
            // before any DMA action has started.
            compiler_fence(SeqCst);

            // Set up the DMA read
            self.0.rxd.ptr.write(|w|
            // We're giving the register a pointer to the stack. Since we're
            // waiting for the UARTE transaction to end before this stack pointer
            // becomes invalid, there's nothing wrong here.
            //
            // The PTR field is a full 32 bits wide and accepts the full range
            // of values.
            unsafe { w.ptr().bits(rx_buffer.as_ptr() as u32) });
            self.0.rxd.maxcnt.write(|w|
            // We're giving it the length of the buffer, so no danger of
            // accessing invalid memory. We have verified that the length of the
            // buffer fits in an `u8`, so the cast to `u8` is also fine.
            //
            // The MAXCNT field is at least 8 bits wide and accepts the full
            // range of values.
            unsafe { w.maxcnt().bits(rx_buffer.len() as _) });

            // Start UARTE Receive transaction.
            self.0.tasks_startrx.write(|w|
            // `1` is a valid value to write to task registers.
            unsafe { w.bits(1) });

            Ok(())
        }

        /// Finalize a UARTE read transaction by clearing the event.
        fn finalize_read(&mut self) {
            // Reset the event, otherwise it will always read `1` from now on.
            self.0.events_endrx.write(|w| w);

            // Conservative compiler fence to prevent optimizations that do not
            // take in to account actions by DMA. The fence has been placed here,
            // after all possible DMA actions have completed.
            compiler_fence(SeqCst);
        }

        /// Stop an unfinished UART read transaction and flush FIFO to DMA buffer.
        fn cancel_read(&mut self) {
            // Stop reception.
            self.0.tasks_stoprx.write(|w| unsafe { w.bits(1) });

            // Wait for the reception to have stopped.
            while self.0.events_rxto.read().bits() == 0 {}

            // Reset the event flag.
            self.0.events_rxto.write(|w| w);

            // Ask UART to flush FIFO to DMA buffer.
            self.0.tasks_flushrx.write(|w| unsafe { w.bits(1) });

            // Wait for the flush to complete.
            while self.0.events_endrx.read().bits() == 0 {}

            // The event flag itself is later reset by `finalize_read`.
        }

        pub fn write_str(&mut self, s: &str) -> fmt::Result {
            // Copy all data into an on-stack buffer so we never try to EasyDMA from
            // flash.
            let buf = &mut [0; 16][..];
            for block in s.as_bytes().chunks(16) {
                buf[..block.len()].copy_from_slice(block);
                self.write(&buf[..block.len()]).map_err(|_| fmt::Error)?;
            }

            Ok(())
        }
    }

    pub struct UarteTx<'a, T> {
        txp: DMABuffer<'a>,
        _marker: core::marker::PhantomData<T>,
    }

    impl<'a, T> UarteTx<'a, T>
    where
        T: Instance,
    {
        pub fn new(txp: DMABuffer<'a>) -> UarteTx<'a, T> {
            let mut tx = UarteTx {
                txp,
                _marker: core::marker::PhantomData,
            };
            //tx.enable_interrupts();
            tx
        }

        fn enable_interrupts(&mut self) {
            let uarte = unsafe { &*T::ptr() };

            uarte.intenset.write(|w| w.endtx().set_bit());
        }

        /// Run TX processing logic - can be run from within an interrupt.
        pub fn process_interrupt(&mut self) {
            let uarte = unsafe { &*T::ptr() };
            log::info!("tx process_interrupt");

            if uarte.events_endtx.read().bits() == 1 {
                log::info!("TX END");
                self.txp.clear();
                uarte.events_endtx.write(|w| w);
            }
        }

        pub fn write(&mut self, tx_buffer: &[u8]) -> Result<(), Error> {
            let uarte = unsafe { &*T::ptr() };
            // We can only DMA out of RAM.
            slice_in_ram_or(tx_buffer, Error::BufferNotInRAM)?;

            // Conservative compiler fence to prevent optimizations that do not
            // take in to account actions by DMA. The fence has been placed here,
            // before any DMA action has started.
            compiler_fence(SeqCst);

            // Reset the events.
            uarte.events_endtx.reset();
            uarte.events_txstopped.reset();

            // Set up the DMA write.
            uarte.txd.ptr.write(|w|
            // We're giving the register a pointer to the stack. Since we're
            // waiting for the UARTE transaction to end before this stack pointer
            // becomes invalid, there's nothing wrong here.
            //
            // The PTR field is a full 32 bits wide and accepts the full range
            // of values.
            unsafe { w.ptr().bits(tx_buffer.as_ptr() as u32) });
            uarte.txd.maxcnt.write(|w|
            // We're giving it the length of the buffer, so no danger of
            // accessing invalid memory. We have verified that the length of the
            // buffer fits in an `u8`, so the cast to `u8` is also fine.
            //
            // The MAXCNT field is 8 bits wide and accepts the full range of
            // values.
            unsafe { w.maxcnt().bits(tx_buffer.len() as _) });

            // Start UARTE Transmit transaction.
            uarte.tasks_starttx.write(|w|
            // `1` is a valid value to write to task registers.
            unsafe { w.bits(1) });

            // Wait for transmission to end.
            let mut endtx;
            let mut txstopped;
            loop {
                endtx = uarte.events_endtx.read().bits() != 0;
                txstopped = uarte.events_txstopped.read().bits() != 0;
                if endtx || txstopped {
                    break;
                }
            }

            // Conservative compiler fence to prevent optimizations that do not
            // take in to account actions by DMA. The fence has been placed here,
            // after all possible DMA actions have completed.
            compiler_fence(SeqCst);

            if txstopped {
                return Err(Error::Transmit);
            }

            // Lower power consumption by disabling the transmitter once we're
            // finished.
            uarte.tasks_stoptx.write(|w|
            // `1` is a valid value to write to task registers.
            unsafe { w.bits(1) });

            Ok(())
        }

        pub fn write_str(&mut self, s: &str) -> core::fmt::Result {
            // Copy all data into an on-stack buffer so we never try to EasyDMA from
            // flash.
            let buf = &mut [0; 16][..];
            for block in s.as_bytes().chunks(16) {
                buf[..block.len()].copy_from_slice(block);
                self.write(&buf[..block.len()])
                    .map_err(|_| core::fmt::Error)?;
            }

            Ok(())
        }
    }

    impl<'a, T> serial::Write<u8> for UarteTx<'a, T>
    where
        T: Instance,
    {
        type Error = Error;
        fn write(&mut self, b: u8) -> nb::Result<(), Self::Error> {
            log::info!("tx write");
            let mut d = [0; 1];
            d[0] = b;
            match self.txp.write(&d[..]) {
                Err(Error::TxBufferTooLong) => return Err(nb::Error::WouldBlock),
                Err(e) => return Err(nb::Error::Other(e)),
                _ => {}
            }
            log::info!("tx write done");
            Ok(())
        }

        fn flush(&mut self) -> nb::Result<(), Self::Error> {
            log::info!("tx flush {} bytes", self.txp.len());
            let uarte = unsafe { &*T::ptr() };

            slice_in_ram_or(self.txp.buffer, Error::BufferNotInRAM)?;

            compiler_fence(SeqCst);

            if self.txp.len() > 0 {
                uarte.events_endtx.reset();
                uarte.events_txstopped.reset();
                uarte
                    .txd
                    .ptr
                    .write(|w| unsafe { w.ptr().bits(self.txp.buffer.as_ptr() as u32) });
                uarte
                    .txd
                    .maxcnt
                    .write(|w| unsafe { w.maxcnt().bits(self.txp.len() as _) });

                uarte.tasks_starttx.write(|w| unsafe { w.bits(1) });

                let mut endtx;
                let mut txstopped;
                loop {
                    endtx = uarte.events_endtx.read().bits() != 0;
                    txstopped = uarte.events_txstopped.read().bits() != 0;
                    if endtx || txstopped {
                        break;
                    }
                }

                // Conservative compiler fence to prevent optimizations that do not
                // take in to account actions by DMA. The fence has been placed here,
                // after all possible DMA actions have completed.
                compiler_fence(SeqCst);

                if txstopped {
                    return Err(nb::Error::Other(Error::Transmit));
                }

                // Lower power consumption by disabling the transmitter once we're
                // finished.
                uarte.tasks_stoptx.write(|w|
            // `1` is a valid value to write to task registers.
                    unsafe { w.bits(1) });
            }
            Ok(())
        }
    }

    pub struct UarteRx<'a, T> {
        rxp: DMABuffer<'a>,
        rp: usize,
        _marker: core::marker::PhantomData<T>,
    }

    impl<'a, T> UarteRx<'a, T>
    where
        T: Instance,
    {
        pub fn new(rxp: DMABuffer<'a>) -> UarteRx<'a, T> {
            let mut rx = UarteRx {
                rxp,
                rp: 0,
                _marker: core::marker::PhantomData,
            };

            //rx.enable_interrupts();
            //rx.prepare_read().unwrap();
            //rx.start_read();

            rx
        }

        fn enable_interrupts(&mut self) {
            let uarte = unsafe { &*T::ptr() };

            uarte.intenset.write(|w| {
                w.endrx()
                    .set_bit()
                    .rxdrdy()
                    .set_bit()
                    .rxstarted()
                    .set_bit()
                    .rxto()
                    .set_bit()
            });
        }

        fn start_read(&mut self) {
            let uarte = unsafe { &*T::ptr() };

            log::info!("rx start_read");
            // Clear previous state

            self.rxp.clear();
            self.rp = 0;

            compiler_fence(SeqCst);
            // Start UARTE Receive transaction
            // `1` is a valid value to write to task registers.
            uarte.tasks_startrx.write(|w| unsafe { w.bits(1) });
        }

        fn prepare_read(&mut self) -> Result<(), Error> {
            log::info!("rx prepare_read");
            // This operation is safe due to type-state programming guaranteeing that the RX and
            // TX are within the driver
            let uarte = unsafe { &*T::ptr() };

            compiler_fence(SeqCst);

            // setup start address
            uarte
                .rxd
                .ptr
                .write(|w| unsafe { w.ptr().bits(self.rxp.buffer.as_ptr() as u32) });
            // setup length
            uarte
                .rxd
                .maxcnt
                .write(|w| unsafe { w.maxcnt().bits(self.rxp.size() as _) });

            Ok(())
        }

        // Process incoming data
        pub fn process_interrupt(&mut self) -> Result<(), Error> {
            log::info!("rx process_interrupt");
            let uarte = unsafe { &*T::ptr() };

            if uarte.events_rxdrdy.read().bits() == 1 {
                log::info!("RXDRDY!");
                // Reset the event, otherwise it will always read `1` from now on.
                uarte.events_rxdrdy.write(|w| w);
            }

            if uarte.events_rxto.read().bits() == 1 {
                log::info!("RXTO!");
                // Tell UARTE to flush FIFO to DMA buffer
                uarte.tasks_flushrx.write(|w| unsafe { w.bits(1) });

                // Reset the event, otherwise it will always read `1` from now on.
                uarte.events_rxto.write(|w| w);
            }

            // check if dma rx transaction has started
            if uarte.events_rxstarted.read().bits() == 1 {
                log::info!("RXSTARTED!");
                // DMA transaction has started
                self.prepare_read()?;

                // Reset the event, otherwise it will always read `1` from now on.
                uarte.events_rxstarted.write(|w| w);
            }

            // check if dma transaction finished
            if uarte.events_endrx.read().bits() == 1 {
                log::info!("ENDRX!");
                // our transaction has finished
                // Read the true number of bytes and set the correct length of the packet
                // before returning it
                let bytes_read = uarte.rxd.amount.read().bits() as u8;
                self.rxp.set_len(bytes_read as usize);

                // Reset the event, otherwise it will always read `1` from now on.
                uarte.events_endrx.write(|w| w);
            }

            log::info!("rx interrupt process done!");
            Ok(())
        }

        pub fn read(&mut self, rx_buffer: &mut [u8]) -> Result<(), Error> {
            let uarte = unsafe { &*T::ptr() };

            compiler_fence(SeqCst);

            slice_in_ram_or(rx_buffer, Error::BufferNotInRAM)?;
            slice_in_ram_or(self.rxp.buffer, Error::BufferNotInRAM)?;

            uarte.events_endrx.reset();
            uarte.events_rxstarted.reset();
            uarte.events_rxdrdy.reset();

            uarte.rxd.ptr.write(|w|
            // We're giving the register a pointer to the stack. Since we're
            // waiting for the UARTE transaction to end before this stack pointer
            // becomes invalid, there's nothing wrong here.
            //
            // The PTR field is a full 32 bits wide and accepts the full range
            // of values.
            unsafe { w.ptr().bits(rx_buffer.as_ptr() as u32) });
            uarte.rxd.maxcnt.write(|w|
            // We're giving it the length of the buffer, so no danger of
            // accessing invalid memory. We have verified that the length of the
            // buffer fits in an `u8`, so the cast to `u8` is also fine.
            //
            // The MAXCNT field is at least 8 bits wide and accepts the full
            // range of values.
            unsafe { w.maxcnt().bits(rx_buffer.len() as _) });

            // Start UARTE Receive transaction.
            uarte.tasks_startrx.write(|w|
            // `1` is a valid value to write to task registers.
            unsafe { w.bits(1) });

            while uarte.events_endrx.read().bits() == 0 {}

            uarte.events_endrx.write(|w| w);

            // Conservative compiler fence to prevent optimizations that do not
            // take in to account actions by DMA. The fence has been placed here,
            // after all possible DMA actions have completed.
            compiler_fence(SeqCst);

            if uarte.rxd.amount.read().bits() != 1 as u32 {
                return Err(Error::Receive);
            }

            Ok(())
        }
    }

    /*
    impl<'a, T> serial::Read<u8> for UarteRx<'a, T>
    where
        T: Instance,
    {
        type Error = Error;
        fn read(&mut self) -> nb::Result<u8, Error> {
            if self.rxp.len() > 0 && self.rp < self.rxp.len() {
                let b = self.rxp.read(self.rp)?;
                self.rp += 1;
                Ok(b)
            } else {
                Err(nb::Error::WouldBlock)
            }
        }
    }*/

    pub struct Pins {
        pub rxd: Pin<Input<Floating>>,
        pub txd: Pin<Output<PushPull>>,
        pub cts: Option<Pin<Input<Floating>>>,
        pub rts: Option<Pin<Output<PushPull>>>,
    }

    #[derive(Debug)]
    pub enum Error {
        TxBufferTooLong,
        RxBufferTooLong,
        Transmit,
        Receive,
        Timeout(usize),
        BufferNotInRAM,
    }

    pub trait Instance: Deref<Target = uarte0::RegisterBlock> + sealed::Sealed {
        fn ptr() -> *const uarte0::RegisterBlock;
    }

    impl Instance for UARTE0 {
        fn ptr() -> *const uarte0::RegisterBlock {
            UARTE0::ptr()
        }
    }

    mod sealed {
        pub trait Sealed {}
    }

    pub mod target_constants {
        // NRF52840 and NRF9160 16 bits 1..0xFFFF
        pub const EASY_DMA_SIZE: usize = 65535;
        // Limits for Easy DMA - it can only read from data ram
        pub const SRAM_LOWER: usize = 0x2000_0000;
        pub const SRAM_UPPER: usize = 0x3000_0000;
        pub const FORCE_COPY_BUFFER_SIZE: usize = 1024;
        const _CHECK_FORCE_COPY_BUFFER_SIZE: usize = EASY_DMA_SIZE - FORCE_COPY_BUFFER_SIZE;
        // ERROR: FORCE_COPY_BUFFER_SIZE must be <= EASY_DMA_SIZE
    }

    impl sealed::Sealed for UARTE0 {}
    pub(crate) fn slice_in_ram(slice: &[u8]) -> bool {
        let ptr = slice.as_ptr() as usize;
        ptr >= target_constants::SRAM_LOWER && (ptr + slice.len()) < target_constants::SRAM_UPPER
    }

    pub(crate) fn slice_in_ram_or<T>(slice: &[u8], err: T) -> Result<(), T> {
        if slice_in_ram(slice) {
            Ok(())
        } else {
            Err(err)
        }
    }
}
*/
