// USB Communication Module

use embassy_stm32::{
    peripherals::{USB_OTG_FS, PA11, PA12},
    usb::{Driver},
};
use embassy_usb::{
    Builder,
    class::cdc_acm::{CdcAcmClass, State},
    Config as UsbConfig,
    UsbDevice,
};
use defmt::{info, error};
use embassy_time::Timer;
use embassy_futures::select::{select, Either};
use static_cell::StaticCell;

use crate::config::{
    USB_BUFFER_SIZE,
    USB_TO_ETH_PIPE, 
    ETH_TO_USB_PIPE,
    Irqs,
};

// USB Buffers (only needed statics)
static USB_STATE: StaticCell<State> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static EP_MEM: StaticCell<[u8; USB_BUFFER_SIZE]> = StaticCell::new();

const USB_VID: u16 = 0xC0DE;
const USB_PID: u16 = 0xCAFE;
const MAX_PACKET_SIZE: usize = 64; // CDC/FS endpoint

const MAX_RETRIES: usize = 5;
const USB_RETRY_DELAY_US: u64 = 100;

#[inline(always)]
pub fn setup_usb(
    usb: USB_OTG_FS,
    pa12: PA12,
    pa11: PA11,
) -> (
    CdcAcmClass<'static, Driver<'static, USB_OTG_FS>>,
    UsbDevice<'static, Driver<'static, USB_OTG_FS>>,
) {
    let ep_buf = EP_MEM.init([0u8; USB_BUFFER_SIZE]);
    let mut driver_config = embassy_stm32::usb::Config::default();
    driver_config.vbus_detection = false;

    let driver = Driver::new_fs(usb, Irqs, pa12, pa11, ep_buf, driver_config);

    let mut usb_cfg = UsbConfig::new(USB_VID, USB_PID);
    usb_cfg.manufacturer = Some("Embassy");
    usb_cfg.product = Some("USB Serial");
    usb_cfg.serial_number = Some("12345678");

    let state = USB_STATE.init(State::new());
    let mut builder = Builder::new(
        driver,
        usb_cfg,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );

    let class = CdcAcmClass::new(&mut builder, state, MAX_PACKET_SIZE as u16);
    let device = builder.build();

    (class, device)
}

#[embassy_executor::task]
pub async fn usb_task(
    mut class: CdcAcmClass<'static, Driver<'static, USB_OTG_FS>>,
) -> ! {
    let mut usb_rx_buf = [0u8; MAX_PACKET_SIZE];
    let mut pipe_buf = [0u8; 512]; // enough for UDP frame size

    loop {
        class.wait_connection().await;
        info!("USB connected");

        loop {
            match select(
                class.read_packet(&mut usb_rx_buf),
                ETH_TO_USB_PIPE.read(&mut pipe_buf),
            ).await {
                Either::First(Ok(n)) => {
                    if n > 0 {
                        USB_TO_ETH_PIPE.write(&usb_rx_buf[..n]).await;
                        info!("USB -> ETH: {} bytes", n);
                    }
                }

                Either::First(Err(_)) => {
                    info!("USB disconnected");
                    break;
                }

                Either::Second(n) => {
                    if n > 0 {
                        info!("ETH -> USB: {} bytes", n);

                        for chunk in pipe_buf[..n].chunks(MAX_PACKET_SIZE) {
                            let mut success = false;

                            for attempt in 1..=MAX_RETRIES {
                                match class.write_packet(chunk).await {
                                    Ok(_) => {
                                        success = true;
                                        break;
                                    }
                                    Err(_) => {
                                        Timer::after_micros(USB_RETRY_DELAY_US * attempt as u64).await;
                                    }
                                }
                            }

                            if !success {
                                error!("USB write failed after {} retries", MAX_RETRIES);
                            }
                        }
                    }
                }
            }
        }
    }
}
