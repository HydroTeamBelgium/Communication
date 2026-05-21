#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::peripherals::*;
use embassy_stm32::{bind_interrupts, can, Config, SharedData};
use embassy_stm32::rcc::{HSIPrescaler, Pll, PllDiv, PllMul, PllPreDiv, PllSource};
use embassy_stm32::rcc;
use embassy_time::{Duration, Timer};
use defmt_rtt as _;
use panic_probe as _;



#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

bind_interrupts!(struct Irqs {
    FDCAN1_IT0 => can::IT0InterruptHandler<FDCAN1>;
    FDCAN1_IT1 => can::IT1InterruptHandler<FDCAN1>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    let mut config = Config::default();

    // config.rcc.hse = Some(rcc::Hse {
    //     freq: embassy_stm32::time::Hertz(25_000_000),
    //     mode: rcc::HseMode::Oscillator,
    // });
    config.rcc.hsi = Some(HSIPrescaler::DIV1);
    config.rcc.pll1 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL60,
        divp: Some(PllDiv::DIV2),
        divq: Some(PllDiv::DIV4), // SPI1 cksel defaults to pll1_q
        divr: Some(PllDiv::DIV2),
    });
    config.rcc.mux.fdcansel = rcc::mux::Fdcansel::PLL1_Q;

    let peripherals = embassy_stm32::init_primary(config, &SHARED_DATA);

    let mut can = can::CanConfigurator::new(peripherals.FDCAN1, peripherals.PD0, peripherals.PD1, Irqs);
    can.set_bitrate(500_000);
    let can = can.into_normal_mode();

    info!("CAN initialized and running!");
    info!("Listening for MaxxECU messages at 500 kbit/s");

    _ = _spawner.spawn(can_reader(can));

    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("Main loop running...");
    }
}

#[embassy_executor::task]
async fn can_reader(mut can: embassy_stm32::can::Can<'static>) -> ! {
    loop {
        match can.read().await {
            Ok(envelope) => {
                let (frame, _ts) = envelope.parts();
                info!("Received CAN frame len={}", frame.data().len());
            }
            Err(err) => {
                error!("CAN read error: {:?}", err);
            }
        }
    }
}