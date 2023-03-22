#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_closure)]

use core::cell::RefCell;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::pwm::simple_pwm::{PwmPin, SimplePwm};
use embassy_stm32::pwm::Channel;
use embassy_stm32::time::{hz, mhz};
use embassy_stm32::Config;
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{AnyPin, Input, Level, Output, Pull, Speed},
};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use futures::future::select;
use futures::pin_mut;
use {defmt_rtt as _, panic_probe as _};

static LED_BLINK: Mutex<CriticalSectionRawMutex, RefCell<Option<Output<AnyPin>>>> =
    Mutex::new(RefCell::new(None));
static LED_STOP_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::task()]
async fn blink() {
    let led_option = LED_BLINK.lock().await;
    let mut led = led_option.replace(None).unwrap();

    let led_ref = &mut led;
    let blink_future = (async move || loop {
        led_ref.set_high();
        Timer::after(Duration::from_millis(500)).await;
        led_ref.set_low();
        Timer::after(Duration::from_millis(500)).await;
    })();
    pin_mut!(blink_future);
    select(LED_STOP_SIGNAL.wait(), blink_future).await;

    led.set_low();
    led_option.replace(Some(led));
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();
    config.rcc.sys_ck = Some(mhz(84));
    config.rcc.hclk = Some(mhz(84));
    config.rcc.pll48 = true;
    let p = embassy_stm32::init(config);

    let button = Input::new(p.PA0, Pull::Down);
    let mut button = ExtiInput::new(button, p.EXTI0);

    {
        let out_pin: AnyPin = p.PG14.into();
        let led = Some(Output::new(out_pin, Level::Low, Speed::Medium));
        let guard = LED_BLINK.lock().await;
        guard.replace(led);
    }

    loop {
        button.wait_for_rising_edge().await;
        info!("Pressed!");

        spawner.spawn(blink()).unwrap();

        button.wait_for_falling_edge().await;
        info!("Released!");

        LED_STOP_SIGNAL.signal(());
    }
}
