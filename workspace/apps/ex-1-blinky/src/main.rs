#![no_std]
#![no_main]

// We need to write our own panic handler
use core::panic::PanicInfo;

// Alias our HAL
use rp235x_hal as hal;

// Import traits for embedded abstractions
use embedded_hal::digital::StatefulOutputPin;

// USB device and Communications Class Device (CDC) support
use usb_device::{class_prelude::*, prelude::*};
use usbd_serial::SerialPort;

// Custom panic handler: just loop forever
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// Copy boot metadata to .start_block so Boot ROM knows how to boot our program
#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

// Set external crystal frequency
const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;

// Main entrypoint (custom defined for embedded targets)
#[hal::entry]
fn main() -> ! {
    // Get ownership of hardware peripherals
    let mut pac = hal::pac::Peripherals::take().unwrap();

    // Set up the watchdog and clocks
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // Single-cycle I/O block (fast GPIO)
    let sio = hal::Sio::new(pac.SIO);

    // Split off ownership of Peripherals struct, set pins to default state
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Configure pin, get ownership of that pin
    let mut led_pin = pins.gpio15.into_push_pull_output();

    // Move ownership of TIMER0 peripheral to create Timer struct
    let timer = hal::Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);

    // Initialize the USB driver
    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USB,
        pac.USB_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    // Configure the USB as CDC
    let mut serial = SerialPort::new(&usb_bus);

    // Create a USB device with a fake VID/PID
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[StringDescriptors::default()
            .manufacturer("Fake company")
            .product("Serial port")
            .serial_number("TEST")])
        .unwrap()
        .device_class(2) // from: https://www.usb.org/defined-class-codes
        .build();
    
    // Read buffer
    let mut rx_buf = [0u8; 64];

    // Superloop
    let mut timestamp = timer.get_counter();
    loop {
        // Poll USB device (needs to be called at least every 10 ms)
        if usb_dev.poll(&mut [&mut serial]) {
            match serial.read(&mut rx_buf) {
                Ok(_count) => {}
                Err(_e) => {}
            }
        }

        // Toggle LED and send a message over USB every second (non-blocking)
        if (timer.get_counter() - timestamp).to_millis() >= 1_000 {
            timestamp = timer.get_counter();
            let _ = led_pin.toggle();
            let _ = serial.write(b"hello!\r\n");
        }
    }
}
