#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{Config, SharedData};
use embassy_stm32::adc::{Adc};
use embassy_time::{Timer};
use {defmt_rtt as _, panic_probe as _};


#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // configure clockes
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        //pll1 is used for the system clock
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSI,
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL50,
            divp: Some(PllDiv::DIV2),
            divq: Some(PllDiv::DIV8), // SPI1 cksel defaults to pll1_q
            divr: None,
        });
        //pll2 is used for the ADC clock
        config.rcc.pll2 = Some(Pll {
            source: PllSource::HSI,
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL50,
            divp: Some(PllDiv::DIV8), // 100mhz
            divq: None,
            divr: None,
        });
        config.rcc.sys = Sysclk::PLL1_P; // 400 Mhz
        config.rcc.ahb_pre = AHBPrescaler::DIV2; // 200 Mhz
        config.rcc.apb1_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb2_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb3_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb4_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.voltage_scale = VoltageScale::Scale1;
        config.rcc.mux.adcsel = mux::Adcsel::PLL2_P;
    }
    let mut p = embassy_stm32::init_primary(config, &SHARED_DATA);    
    info!("Hello World!");
    let mut adc = Adc::new(p.ADC2);
    let mut vrefint_channel = adc.enable_vrefint();
    const VREFINT_CAL_ADDR: *const u16 = 0x1FF1E860 as *const u16;
    let vrefint_cal = unsafe{core::ptr::read(VREFINT_CAL_ADDR)};
    info!("vrefint_cal: {}", vrefint_cal);


    loop {
        
        let vrefint = adc.blocking_read(&mut vrefint_channel);
        info!("vrefint: {}", vrefint);
        let measured = adc.blocking_read(&mut p.PA3);
        let vdda = 0.33 * vrefint_cal as f32 / vrefint as f32;
        let voltage = (measured as f32 / 16383.0) * vdda;
        info!("measured, voltage : {} {}", measured, voltage);
        Timer::after_millis(500).await;
    }
}