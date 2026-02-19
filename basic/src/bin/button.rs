#![no_std]
#![no_main]

use defmt::*;
use core::mem::MaybeUninit;
use embassy_stm32::{Config, SharedData};
use embassy_executor::Spawner;
use embassy_stm32::exti::{ExtiInput};
use embassy_stm32::gpio::Pull;
use {defmt_rtt as _, panic_probe as _};

#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();


#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let config = Config::default();
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);    
    info!("Hello World!");

    // define as an External Interrupt Input pin
    let mut button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);


    info!("Press the USER button...");

    loop {
        button.wait_for_rising_edge().await;
        info!("Pressed!");
        button.wait_for_falling_edge().await;
        info!("Released!");
    }
}