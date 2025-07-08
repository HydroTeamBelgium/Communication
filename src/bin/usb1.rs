use embassy_stm32::{
    peripherals,
    usb::{self, Driver},
};
use embassy_usb::{
    Builder,
    class::cdc_acm::{CdcAcmClass, State},
    Config as UsbConfig,
    UsbDevice,
};
use defmt::{info, error};
use embassy_futures::select::{select, Either};

use static_cell::StaticCell;

use crate::config1::{
    USB_BUFFER_SIZE,
    USB_TO_ETH_PIPE, 
    ETH_TO_USB_PIPE,
    Irqs
};

// USB Buffers
static EP_OUT_BUFFER: StaticCell<[u8; USB_BUFFER_SIZE]> = StaticCell::new();
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static STATE: StaticCell<State> = StaticCell::new();


/// Initializes the USB CDC ACM (Serial) interface.
pub fn setup_usb(
    p: peripherals::USB_OTG_FS,
    pa12: peripherals::PA12,
    pa11: peripherals::PA11,
) -> (
    CdcAcmClass<'static, Driver<'static, peripherals::USB_OTG_FS>>,
    UsbDevice<'static, Driver<'static, peripherals::USB_OTG_FS>>,
) {
    let ep_out_buffer = EP_OUT_BUFFER.init([0u8; USB_BUFFER_SIZE]);
    let mut usb_config = usb::Config::default();
    usb_config.vbus_detection = false;

    let driver = Driver::new_fs(p, Irqs, pa12, pa11, &mut *ep_out_buffer, usb_config);

    let mut config = UsbConfig::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB Serial");
    config.serial_number = Some("12345678");

    let config_descriptor = CONFIG_DESCRIPTOR.init([0; 256]);
    let bos_descriptor = BOS_DESCRIPTOR.init([0; 256]);
    let control_buf = CONTROL_BUF.init([0; 64]);
    let state = STATE.init(State::new());

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor[..],
        &mut bos_descriptor[..],
        &mut [],
        &mut control_buf[..],
    );

    let class = CdcAcmClass::new(&mut builder, state, 64);
    let usb = builder.build();
    (class, usb)
}

/// USB Communication Task
///
/// - Reads data from USB serial port.
/// - Writes data to a pipe for the UDP task.
#[embassy_executor::task]
pub async fn usb_task(
    mut class: CdcAcmClass<'static, Driver<'static, peripherals::USB_OTG_FS>>,
) -> ! {
    const CHUNK_SIZE: usize = 64;
    let mut usb_rx_buf = [0u8; CHUNK_SIZE];
    let mut pipe_rx_buf = [0u8; 512];

    loop {
        class.wait_connection().await;
        info!("USB connected");

        loop {
            match select(
                class.read_packet(&mut usb_rx_buf),
                ETH_TO_USB_PIPE.read(&mut pipe_rx_buf),
            )
            .await
            {
                Either::First(Ok(n)) => {
                    if n > 0 {
                        info!("Received {} bytes over USB", n);
                        USB_TO_ETH_PIPE.write(&usb_rx_buf[..n]).await;
                    }
                }
                Either::First(Err(_)) => {
                    info!("USB disconnected");
                    break;
                }
                Either::Second(n) => {
                    if n > 0 {
                        info!("Forwarding {} bytes over USB (chunked)", n);
                        for chunk in pipe_rx_buf[..n].chunks(CHUNK_SIZE) {
                            let mut attempts = 0;
                            while let Err(e) = class.write_packet(chunk).await {
                                attempts += 1;
                                if attempts > 3 {
                                    error!("USB write failed after retries: {:?}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
