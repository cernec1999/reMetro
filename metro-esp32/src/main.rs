#![no_std]
#![no_main]

use bt_hci::controller::ExternalController;
use embassy_executor::Spawner;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Pin;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hub75::framebuffer::plain::DmaFrameBuffer;
use esp_hub75::framebuffer::{compute_frame_count, compute_rows};
use esp_hub75::{Hub75, Hub75Pins16};
use esp_radio::ble::controller::BleConnector;
use metro_render::scene::train_predictions::{TrainPrediction, TrainPredictions};
use trouble_host::prelude::*;
use u8g2_fonts::{Font, FontRenderer};

extern crate alloc;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

// Display configuration
const ROWS: usize = 32;
const COLS: usize = 128;
const BITS: u8 = 4;
const NROWS: usize = compute_rows(ROWS);
const FRAME_COUNT: usize = compute_frame_count(BITS);
type FBType = DmaFrameBuffer<ROWS, COLS, NROWS, BITS, FRAME_COUNT>;

// BLE configuration
const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

pub struct MetroFont;
impl Font for MetroFont {
    const DATA: &'static [u8] =
        include_bytes!("../../metro-font-builder/sample_output/pims_3.u8g2font");
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // Initialize ESP32 hardware
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let (_, tx_descriptors) = esp_hal::dma_descriptors!(0, FBType::dma_buffer_size_bytes());

    // Configure memory allocators
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 73744);
    esp_alloc::heap_allocator!(size: 64 * 1024);

    // Initialize timer and RTOS
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    // Initialize radio (WiFi and BLE)
    let radio_init = esp_radio::init().expect("Failed to initialize radio");
    let (_wifi_controller, _interfaces) =
        esp_radio::wifi::new(&radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize WiFi");

    let transport = BleConnector::new(&radio_init, peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 20>::new(transport);
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let _ble_stack = trouble_host::new(ble_controller, &mut resources);

    // Configure HUB75 display pins
    let pins = Hub75Pins16 {
        red1: peripherals.GPIO42.degrade(),
        blu1: peripherals.GPIO40.degrade(),
        red2: peripherals.GPIO38.degrade(),
        blu2: peripherals.GPIO37.degrade(),
        grn1: peripherals.GPIO41.degrade(),
        grn2: peripherals.GPIO39.degrade(),
        addr0: peripherals.GPIO45.degrade(),
        addr1: peripherals.GPIO36.degrade(),
        addr2: peripherals.GPIO48.degrade(),
        addr3: peripherals.GPIO35.degrade(),
        addr4: peripherals.GPIO21.degrade(),
        blank: peripherals.GPIO14.degrade(),
        clock: peripherals.GPIO2.degrade(),
        latch: peripherals.GPIO47.degrade(),
    };

    // Initialize display
    let mut hub75 = Hub75::new_async(
        peripherals.LCD_CAM,
        pins,
        peripherals.DMA_CH0,
        tx_descriptors,
        Rate::from_mhz(20),
    )
    .expect("Failed to create HUB75 display");

    let mut fb = FBType::new();
    let font = FontRenderer::new::<MetroFont>().with_line_height(16);

    // Sample train prediction data
    let predictions: &[TrainPrediction] = &[
        TrainPrediction {
            line: "SV",
            cars: "8",
            destination: "Ashburn",
            arrival_in_minutes: "1",
        },
        TrainPrediction {
            line: "OR",
            cars: "6",
            destination: "Vienna",
            arrival_in_minutes: "3",
        },
        TrainPrediction {
            line: "BL",
            cars: "6",
            destination: "Franconia",
            arrival_in_minutes: "11",
        },
    ];

    // Render the metro sign
    let mut scene = TrainPredictions { predictions, font };
    scene.render(&mut fb);

    // Future: spawn tasks for WiFi/BLE data updates
    let _ = spawner;

    // Display loop
    loop {
        let xfer = hub75
            .render(&fb)
            .map_err(|(e, _hub75)| e)
            .expect("Failed to start render");

        let (result, new_hub75) = xfer.wait();
        hub75 = new_hub75;
        result.expect("Failed to complete render");
    }
}
