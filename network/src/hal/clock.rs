//! Clock configuration for STM32H755
//!
//! Provides the known-good clock configuration used by the standalone MaxxECU
//! CAN receiver example.

use defmt::info;
use embassy_stm32::Config;
use embassy_stm32::rcc::*;

/// Configure clocks for networking using the known-good MaxxECU CAN timing.
///
/// This matches the standalone `can_receiver_maxxecu.rs` setup:
/// - HSI as the PLL1 source
/// - PLL1_Q as the FDCAN kernel clock
/// - No HSE dependency
pub fn configure_clock_full(config: &mut Config) {
    config.rcc.hsi = Some(HSIPrescaler::DIV1);
    config.rcc.mux.fdcansel = mux::Fdcansel::PLL1_Q;

    
    // PLL1: matches the standalone MaxxECU receiver example.
    config.rcc.pll1 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL60,
        divp: Some(PllDiv::DIV2),
        divq: Some(PllDiv::DIV4),
        divr: Some(PllDiv::DIV2),
    });

    info!("Clock config: HSI/4*60, FDCAN kernel=PLL1_Q");
    info!("Clock debug: this matches the standalone can_receiver_maxxecu.rs timing path");
}

/// Configure clocks for networking only (no ADC)
///
/// Uses the same known-good PLL1/Q path as the CAN receiver example.
pub fn configure_clock_network_only(config: &mut Config) {
    config.rcc.hsi = Some(HSIPrescaler::DIV1);
    
    config.rcc.pll1 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL60,
        divp: Some(PllDiv::DIV2),
        divq: Some(PllDiv::DIV4),
        divr: Some(PllDiv::DIV2),
    });

    info!("Clock config: network-only mode uses the same HSI/PLL1 clock path as the CAN receiver example");
}
