//! Clock configuration for STM32H755
//!
//! Provides standard clock configurations for the Nucleo boards.

use embassy_stm32::Config;
use embassy_stm32::rcc::*;

/// Configure clocks for networking (400MHz system, 100MHz ADC)
///
/// This configuration enables:
/// - PLL1: System clock at 400MHz (AHB at 200MHz, APBx at 100MHz)
/// - PLL2: ADC clock at 100MHz
/// - HSI48 for USB
pub fn configure_clock_full(config: &mut Config) {
    config.rcc.hsi = Some(HSIPrescaler::DIV1);
    config.rcc.csi = true;

    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::Hertz(25_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.mux.fdcansel = mux::Fdcansel::HSE;

    
    // PLL1: System clock
    config.rcc.pll1 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL50,
        divp: Some(PllDiv::DIV2),  // 400 MHz
        divq: Some(PllDiv::DIV8),  // 100 MHz for SPI
        divr: None,
    });
    
    // PLL2: ADC clock
    config.rcc.pll2 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL50,
        divp: Some(PllDiv::DIV8),  // 100 MHz
        divq: None,
        divr: None,
    });
    
    config.rcc.sys = Sysclk::PLL1_P;           // 400 MHz
    config.rcc.ahb_pre = AHBPrescaler::DIV2;   // 200 MHz
    config.rcc.apb1_pre = APBPrescaler::DIV2;  // 100 MHz
    config.rcc.apb2_pre = APBPrescaler::DIV2;  // 100 MHz
    config.rcc.apb3_pre = APBPrescaler::DIV2;  // 100 MHz
    config.rcc.apb4_pre = APBPrescaler::DIV2;  // 100 MHz
    config.rcc.voltage_scale = VoltageScale::Scale1;
    config.rcc.supply_config = SupplyConfig::DirectSMPS;
    config.rcc.mux.usbsel = mux::Usbsel::HSI48;
    config.rcc.mux.adcsel = mux::Adcsel::PLL2_P;
}

/// Configure clocks for networking only (no ADC)
///
/// Lighter configuration without PLL2 for ADC.
pub fn configure_clock_network_only(config: &mut Config) {
    config.rcc.hsi = Some(HSIPrescaler::DIV1);
    config.rcc.csi = true;
    
    config.rcc.pll1 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL50,
        divp: Some(PllDiv::DIV2),
        divq: Some(PllDiv::DIV8),
        divr: None,
    });
    
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV2;
    config.rcc.apb1_pre = APBPrescaler::DIV2;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.apb3_pre = APBPrescaler::DIV2;
    config.rcc.apb4_pre = APBPrescaler::DIV2;
    config.rcc.voltage_scale = VoltageScale::Scale1;
    config.rcc.supply_config = SupplyConfig::DirectSMPS;
    config.rcc.mux.usbsel = mux::Usbsel::HSI48;
}
