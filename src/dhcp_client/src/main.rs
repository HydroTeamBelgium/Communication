#![no_std]
#![no_main]

use embassy::time::{Duration, Timer};
use embassy_net::dhcp::Dhcpv4Client;
use embassy_net::Ethernet;
use embassy::executor::Spawner;
use embassy_stm32::Peripherals;
use cortex_m_rt::entry;
use stm32h7xx_hal::pac;
use embassy_stm32::network::EthernetPhy;
use embassy_net::Stack;

#[embassy::main]
async fn main(spawner: Spawner) {
    // Initialisatie van de microcontroller
    let p = Peripherals::take().unwrap();

    // Configureer je Ethernet interface
    let ethernet = Ethernet::new(p.ethernet_mac, p.ethernet_dma);
    let mut stack = Stack::new(ethernet);
    
    // Initialiseer DHCP client
    let mut dhcp_client = Dhcpv4Client::new(&mut stack);

    // Probeer een DHCP lease te verkrijgen
    match dhcp_client.run().await {
        Ok(_) => {
            // IP-adres succesvol verkregen via DHCP
            let ip = dhcp_client.ipv4_address();
            defmt::info!("Verkregen IP: {}", ip);
        }
        Err(e) => {
            defmt::error!("Fout bij DHCP: {:?}", e);
        }
    }

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
