#![no_std]
#![no_main]

use core::mem::MaybeUninit;

use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::usb::{Driver, Instance};
use embassy_stm32::{bind_interrupts, peripherals, usb, Config, SharedData};
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use embassy_usb::Builder;
use {defmt_rtt as _, panic_probe as _};

#[link_section = ".ram_d3.shared_data"]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Starting USB serial on STM32H755ZI...");

    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        config.rcc.hsi48 = Some(Hsi48Config { sync_from_usb: true });
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

    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // LED assignments
    let mut led_connect = Output::new(p.PB0, Level::Low, Speed::Low); // LD2 Blue
    let mut led_recv = Output::new(p.PE1, Level::Low, Speed::Low);    // LD1 Green
    let mut led_send = Output::new(p.PB14, Level::Low, Speed::Low);   // LD3 Red

    let mut ep_out_buffer = [0u8; 256];
    let mut usb_config = embassy_stm32::usb::Config::default();
    usb_config.vbus_detection = false;

    let driver = Driver::new_fs(p.USB_OTG_FS, Irqs, p.PA12, p.PA11, &mut ep_out_buffer, usb_config);

    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB Serial");
    config.serial_number = Some("12345678");

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );

    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);
    let mut usb = builder.build();

    let usb_fut = usb.run();

    let echo_fut = async {
        loop {
            class.wait_connection().await;
            info!("USB connected!");
            led_connect.set_high(); // LD2 ON

            let _ = echo(&mut class, &mut led_recv, &mut led_send).await;

            info!("USB disconnected!");
            led_connect.set_low(); // LD2 OFF
        }
    };

    join(usb_fut, echo_fut).await;
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(e: EndpointError) -> Self {
        match e {
            EndpointError::BufferOverflow => defmt::panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

async fn echo<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
    led_recv: &mut Output<'_>,
    led_send: &mut Output<'_>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];

    loop {
        let n = class.read_packet(&mut buf).await?;
        let data = &buf[..n];
        info!("Received {:x}", data);

        // Blink receive LED
        led_recv.set_high();
        Timer::after_millis(50).await;
        led_recv.set_low(); 

        class.write_packet(data).await?;

        // Blink send LED
        led_send.set_high();
        Timer::after_millis(50).await;
        led_send.set_low();
    }
}
