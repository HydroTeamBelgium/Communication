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
use embassy_time::Instant;
use embedded_can::Id;
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
    let mut last_ts = Instant::now();
    let mut frames_in_second: u32 = 0;
    let mut second_start = Instant::now();

    loop {
        match can.read().await {
            Ok(envelope) => {
                let (frame, ts) = envelope.parts();
                let data = frame.data();
                // extract numeric id
                let id_val: u32 = match frame.id() {
                    Id::Standard(id) => id.as_raw() as u32,
                    Id::Extended(id) => id.as_raw(),
                };

                // timing
                let delta = (ts - last_ts).as_millis();
                last_ts = ts;

                // per-second counter
                frames_in_second += 1;
                if (ts - second_start).as_secs() >= 1 {
                    info!("Frames last second: {}", frames_in_second);
                    frames_in_second = 0;
                    second_start = ts;
                }

                // safe hex dump for variable length (include ID)
                match data.len() {
                    0 => info!("Rx ID=0x{:03X} len=0 delta={}ms", id_val, delta),
                    1 => info!("Rx ID=0x{:03X} len=1 delta={}ms: {:02x}", id_val, delta, data[0]),
                    2 => info!("Rx ID=0x{:03X} len=2 delta={}ms: {:02x} {:02x}", id_val, delta, data[0], data[1]),
                    3 => info!("Rx ID=0x{:03X} len=3 delta={}ms: {:02x} {:02x} {:02x}", id_val, delta, data[0], data[1], data[2]),
                    4 => info!("Rx ID=0x{:03X} len=4 delta={}ms: {:02x} {:02x} {:02x} {:02x}", id_val, delta, data[0], data[1], data[2], data[3]),
                    5 => info!("Rx ID=0x{:03X} len=5 delta={}ms: {:02x} {:02x} {:02x} {:02x} {:02x}", id_val, delta, data[0], data[1], data[2], data[3], data[4]),
                    6 => info!("Rx ID=0x{:03X} len=6 delta={}ms: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}", id_val, delta, data[0], data[1], data[2], data[3], data[4], data[5]),
                    7 => info!("Rx ID=0x{:03X} len=7 delta={}ms: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}", id_val, delta, data[0], data[1], data[2], data[3], data[4], data[5], data[6]),
                    _ => info!("Rx ID=0x{:03X} len=8 delta={}ms: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}", id_val, delta, data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]),
                }
            }
            Err(err) => {
                error!("CAN read error: {:?}", err);
            }
        }
    }
}