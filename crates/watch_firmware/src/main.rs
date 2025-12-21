#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::spim;
use embassy_time::Timer;
use embedded_hal_bus::spi::ExclusiveDevice;
use epd_waveshare::{self, prelude::WaveshareDisplay};
use {defmt_rtt as _, panic_probe as _};
embassy_nrf::bind_interrupts!(struct Irqs {
    SPI2 => spim::InterruptHandler<embassy_nrf::peripherals::SPI2>;
});
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());
    let mut led_red = Output::new(p.P0_26, Level::Low, OutputDrive::Standard);
    let mut led_green = Output::new(p.P0_30, Level::Low, OutputDrive::Standard);
    let mut led_blue = Output::new(p.P0_06, Level::Low, OutputDrive::Standard);

    let mut config = spim::Config::default();
    config.frequency = spim::Frequency::M4;

    // 1. Initialize SPI Bus
    let spi_bus = spim::Spim::new_txonly(
        p.SPI2,  //
        Irqs,    //
        p.P1_13, // SCK (D8) -> CLK
        p.P1_15, // MOSI (D10) -> DIN
        config,
    );

    // 2. CS Pin
    let cs = Output::new(p.P0_02, Level::High, OutputDrive::Standard); // D0

    // 3. Other Pins
    let busy = Input::new(p.P0_03, Pull::None); // D1
    let dc = Output::new(p.P0_28, Level::Low, OutputDrive::Standard); // D2
    let rst = Output::new(p.P0_29, Level::Low, OutputDrive::Standard); // D3

    let mut spi_device = ExclusiveDevice::new(spi_bus, cs, embassy_time::Delay);

    let mut delay = embassy_time::Delay;

    led_red.set_low();
    let mut epd =
        epd_waveshare::epd1in54_v2::Epd1in54::new(&mut spi_device, busy, dc, rst, &mut delay, None)
            .unwrap();

    let mut buffer = [0x00u8; 5000]; // 0xFF is White

    // Pixel (x:0, y:0) is the highest bit of the first byte.
    // 0 is Black, 1 is White.
    buffer[0] = 0x7F; // 0111 1111 (First pixel black)
    buffer[10] = 0xFF;
    buffer[2000] = 0xFF;
    buffer[3333] = 0xFF;

    led_green.set_low();
    // Push buffer to display
    epd.clear_frame(&mut spi_device, &mut delay);
    epd.update_frame(&mut spi_device, &buffer, &mut delay)
        .unwrap();
    epd.display_frame(&mut spi_device, &mut delay).unwrap();

    led_blue.set_low();
    // TODO: remember epd.sleep to prevent ghosting

    loop {
        led_red.set_high();
        Timer::after_millis(1000).await;
        led_red.set_low();
        Timer::after_millis(1000).await;
        epd.update_frame(&mut spi_device, &buffer, &mut delay)
            .unwrap();
        epd.display_frame(&mut spi_device, &mut delay).unwrap();
        Timer::after_millis(2000).await;
        //  epd.clear_frame(&mut spi_device, &mut delay);
    }
}
