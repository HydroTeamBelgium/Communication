//! Ethernet setup helpers
//!
//! Provides helper functions and macros for Ethernet initialization.

use embassy_net::Ipv4Cidr;

use crate::config::{BoardConfig, NETWORK_GATEWAY_IP, NETWORK_SUBNET_PREFIX};

/// Create static network configuration from board config
pub fn create_net_config(board: &BoardConfig) -> embassy_net::Config {
    embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(board.ip, NETWORK_SUBNET_PREFIX),
        dns_servers: heapless::Vec::new(),
        gateway: Some(NETWORK_GATEWAY_IP),
    })
}

/// Macro to declare all static network resources
///
/// Usage:
/// ```
/// declare_net_statics!();
/// ```
/// This creates PACKETS, RESOURCES, and STACK static cells.
#[macro_export]
macro_rules! declare_net_statics {
    () => {
        static PACKETS: ::static_cell::StaticCell<
            ::embassy_stm32::eth::PacketQueue<8, 8>,
        > = ::static_cell::StaticCell::new();
        
        static RESOURCES: ::static_cell::StaticCell<
            ::embassy_net::StackResources<8>,
        > = ::static_cell::StaticCell::new();
        
        static STACK: ::static_cell::StaticCell<
            ::embassy_net::Stack<'static>,
        > = ::static_cell::StaticCell::new();
    };
}

/// Macro to declare shared data for dual-core STM32H755
#[macro_export]
macro_rules! declare_shared_data {
    () => {
        #[unsafe(link_section = ".ram_d3.shared_data")]
        static SHARED_DATA: ::core::mem::MaybeUninit<::embassy_stm32::SharedData> = 
            ::core::mem::MaybeUninit::uninit();
    };
}

/// Macro to bind Ethernet and RNG interrupts
#[macro_export]
macro_rules! bind_eth_interrupts {
    () => {
        ::embassy_stm32::bind_interrupts!(struct Irqs {
            ETH => ::embassy_stm32::eth::InterruptHandler;
            HASH_RNG => ::embassy_stm32::rng::InterruptHandler<::embassy_stm32::peripherals::RNG>;
        });
    };
}
