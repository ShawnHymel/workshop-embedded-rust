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

// Bring GPIO structs/functions into scope
use hal::gpio::{FunctionI2C, Pin};

// I2C structs/functions
use embedded_hal::i2c::I2c;
use embedded_hal::digital::InputPin;

// Used for the rate/frequency type
use hal::fugit::RateExtU32;

// For working with non-heap strings
use core::fmt::Write;
use heapless::String;

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

// TMP102 constants
const TMP102_ADDR: u8 = 0x48; // Device address on bus
const TMP102_REG_TEMP: u8 = 0x0; // Address of temperature register

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

    // Configure button pin
    let mut btn_pin = pins.gpio14.into_pull_up_input();

    // Configure I2C pins
    let sda_pin: Pin<hal::gpio::bank0::Gpio18, FunctionI2C, hal::gpio::PullNone> = pins.gpio18.reconfigure();
    let scl_pin: Pin<hal::gpio::bank0::Gpio19, FunctionI2C, hal::gpio::PullNone> = pins.gpio19.reconfigure();

    // Initialize and take ownership of the I2C peripheral
    let mut i2c = hal::I2C::i2c1_with_external_pull_up(
        pac.I2C1,
        sda_pin,
        scl_pin,
        100.kHz(),
        &mut pac.RESETS,
        &clocks.system_clock,
    );

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
    let mut output: String<64> = String::new();

    // Superloop
    let mut prev_pressed = false;
    loop {
        // Poll USB device (needs to be called at least every 10 ms)
        if usb_dev.poll(&mut [&mut serial]) {
            match serial.read(&mut rx_buf) {
                Ok(_count) => {}
                Err(_e) => {}
            }
        }

        // Get button state
        let btn_pressed: bool = match btn_pin.is_low() {
            Ok(state) => state,
            Err(_e) => false,
        };

        // Determine if the button was pressed
        if btn_pressed && (!prev_pressed) {
            // Toggle LED to show that we are reading
            let _ = led_pin.toggle();

            // Read from sensor
            let result = i2c.write_read(TMP102_ADDR, &[TMP102_REG_TEMP], &mut rx_buf);
            if result.is_err() {
                let _ = serial.write(b"ERROR: Could not read temperature\r\n");
                continue;
            }

            // Convert raw reading (signed 12-bit value) into Celsius
            let temp_raw = ((rx_buf[0] as u16) << 8) | (rx_buf[1] as u16);
            let temp_signed = (temp_raw as i16) >> 4;
            let temp_c = (temp_signed as f32) * 0.0625;

            // Print out value
            output.clear();
            write!(&mut output, "Temperature: {:.2} deg C\r\n", temp_c).unwrap();
            let _ = serial.write(output.as_bytes());
        }

        // Save button pressed state for next iteration
        prev_pressed = btn_pressed;
    }
}
