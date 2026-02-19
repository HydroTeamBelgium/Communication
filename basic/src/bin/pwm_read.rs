#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::time::{khz};
use embassy_stm32::timer::pwm_input::PwmInput;
use embassy_stm32::{Config, SharedData, Peri, bind_interrupts, peripherals, timer};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};


#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

bind_interrupts!(struct Irqs {
    TIM2 => timer::CaptureCompareInterruptHandler<peripherals::TIM2>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let config = Config::default();
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);    
    info!("Hello World!");


    let mut pwm_input = PwmInput::new_ch1(p.TIM2, p.PA0, Pull::None, khz(50));
    pwm_input.enable();

    loop {
        Timer::after_millis(500).await;
        let period = pwm_input.get_period_ticks();
        let width = pwm_input.get_width_ticks();
        let duty_cycle = pwm_input.get_duty_cycle();
        info!(
            "period ticks: {} width ticks: {} duty cycle: {}",
            period, width, duty_cycle
        );
    }
}