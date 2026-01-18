// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::cell::Cell;
use core::task::Waker;
use cortex_m::interrupt::Mutex;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::peripheral::SYST;
use cortex_m_rt::exception;
use embassy_time_driver::Driver;

struct SystickDriver;

static TICKS: Mutex<Cell<u64>> = Mutex::new(Cell::new(0));
embassy_time_driver::time_driver_impl!(static DRIVER: SystickDriver = SystickDriver);

impl Driver for SystickDriver {
    fn now(&self) -> u64 {
        cortex_m::interrupt::free(|cs| TICKS.borrow(cs).get())
    }

    fn schedule_wake(&self, _at: u64, _waker: &Waker) {
        // Rely on periodic SysTick interrupt to wake executor
        // Wake immediately for QEMU compatibility
        _waker.wake_by_ref();
    }
}

#[exception]
fn SysTick() {
    cortex_m::interrupt::free(|cs| {
        let ticks = TICKS.borrow(cs);
        let t = ticks.get() + 1;
        ticks.set(t);
    });
}

pub fn init(syst: &mut SYST) {
    syst.set_clock_source(SystClkSource::Core);
    // MPS2-AN386 runs at 25MHz. 25_000_000 / 1000 = 25_000 ticks = 1ms
    syst.set_reload(25_000 - 1);
    syst.clear_current();
    syst.enable_counter();
    syst.enable_interrupt();
}
