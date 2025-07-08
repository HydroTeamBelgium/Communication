use embassy_net::Ipv4Address;
use embassy_sync::{
    blocking_mutex::raw::ThreadModeRawMutex, 
    pipe::Pipe,
};

use embassy_stm32::{
    rng::{InterruptHandler as RngInterruptHandler},
    eth,
    bind_interrupts,
    SharedData,
    peripherals,
    Config,
    rcc::*,
};

use core::mem::MaybeUninit;

/// Network Configuration
pub const NETWORK_LOCAL_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 61);
pub const NETWORK_GATEWAY_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 1);
pub const NETWORK_UDP_PORT: u16 = 4321;

/// Buffer Sizes
pub const USB_BUFFER_SIZE: usize = 1024;

// Inter-task Communication
pub static USB_TO_ETH_PIPE: Pipe<ThreadModeRawMutex, 4096> = Pipe::new();
pub static ETH_TO_USB_PIPE: Pipe<ThreadModeRawMutex, 16384> = Pipe::new();

// Hardware Shared Data
#[link_section = ".ram_d3.shared_data"]
pub static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

bind_interrupts!(pub struct Irqs {
    ETH => eth::InterruptHandler;
    OTG_FS => embassy_stm32::usb::InterruptHandler<peripherals::USB_OTG_FS>;
    HASH_RNG => RngInterruptHandler<peripherals::RNG>;
});

/// Validates the configuration values at startup.
pub fn validate_config() {
    assert!(
        USB_BUFFER_SIZE >= 64,
        "USB_BUFFER_SIZE must be at least 64"
    );
}


/// Configures the STM32 clock tree for optimal performance.
pub fn configure_clock(config: &mut Config) {
    
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
