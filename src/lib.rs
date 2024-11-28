//! [![github]](https://github.com/NotQuiteApex/picoboot-rs) &ensp; [![crates-io]](https://crates.io/crates/picoboot-rs) &ensp; [![docs-rs]](https://docs.rs/picoboot-rs)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! Connecting to and communicating with a Raspberry Pi microcontroller in BOOTSEL mode over USB.
//!
//! <br>
//!
//! PICOBOOT is a USB interface provided by Raspberry Pi microcontrollers when
//! in BOOTSEL mode. Normally, firmware for a Raspberry Pi microcontroller is
//! loaded over a USB Mass Storage Device interface, appearing as a 128MB flash
//! drive to the computer. The PICOBOOT interface is (usually) also active
//! during this time, and can be used for more advanced management of the
//! microcontroller device.
//!
//! # Example
//!
//! Flash a UF2 to a Pico device!
//!
//! ```rust
//! use picoboot_rs::{
//!     PicobootConnection, TargetID, PICO_FLASH_START, PICO_PAGE_SIZE, PICO_SECTOR_SIZE,
//!     PICO_STACK_POINTER,
//! };
//!
//! use rusb::Context;
//! use uf2_decode::convert_from_uf2;
//!
//! // creates a vector of vectors of u8's that map to flash pages sequentially
//! fn uf2_pages(bytes: Vec<u8>) -> Vec<Vec<u8>> {
//!     // loads the uf2 file into a binary
//!     let fw = convert_from_uf2(&bytes).expect("failed to parse uf2").0;
//!
//!     let mut fw_pages: Vec<Vec<u8>> = vec![];
//!     let len = fw.len();
//!
//!     // splits the binary into sequential pages
//!     for i in (0..len).step_by(PICO_PAGE_SIZE as usize) {
//!         let size = std::cmp::min(len - i, PICO_PAGE_SIZE as usize);
//!         let mut page = fw[i..i + size].to_vec();
//!         page.resize(PICO_PAGE_SIZE as usize, 0);
//!         fw_pages.push(page);
//!     }
//!
//!     fw_pages
//! }
//!
//! fn main() {
//!     match Context::new() {
//!         Ok(ctx) => {
//!             // create connection object
//!             let mut conn = PicobootConnection::new(ctx, None)
//!                 .expect("failed to connect to PICOBOOT interface");
//!
//!             conn.reset_interface().expect("failed to reset interface");
//!             conn.access_exclusive_eject().expect("failed to claim access");
//!             conn.exit_xip().expect("failed to exit from xip mode");
//!
//!             // firmware in a big vector of u8's
//!             let fw = std::fs::read("blink.uf2").expect("failed to read firmware");
//!             let fw_pages = uf2_pages(fw);
//!
//!             // erase space on flash
//!             for (i, _) in fw_pages.iter().enumerate() {
//!                 let addr = (i as u32) * PICO_PAGE_SIZE + PICO_FLASH_START;
//!                 if (addr % PICO_SECTOR_SIZE) == 0 {
//!                     conn.flash_erase(addr, PICO_SECTOR_SIZE)
//!                         .expect("failed to erase flash");
//!                 }
//!             }
//!
//!             for (i, page) in fw_pages.iter().enumerate() {
//!                 let addr = (i as u32) * PICO_PAGE_SIZE + PICO_FLASH_START;
//!                 let size = PICO_PAGE_SIZE as u32;
//!
//!                 // write page to flash
//!                 conn.flash_write(addr, page).expect("failed to write flash");
//!
//!                 // confirm flash write was successful
//!                 let read = conn.flash_read(addr, size).expect("failed to read flash");
//!                 let matching = page.iter().zip(&read).all(|(&a, &b)| a == b);
//!                 assert!(matching, "page does not match flash");
//!             }
//!
//!             // reboot device to start firmware
//!             let delay = 500; // in milliseconds
//!             match conn.get_device_type() {
//!                 TargetID::Rp2040 => {
//!                     conn.reboot(0x0, PICO_STACK_POINTER, delay)
//!                         .expect("failed to reboot device");
//!                 }
//!                 TargetID::Rp2350 => conn.reboot2_normal(delay)
//!                     .expect("failed to reboot device"),
//!             }
//!         }
//!         Err(e) => panic!("Could not initialize libusb: {}", e),
//!     }
//! }
//! ```

/// RP MCU flash page size (for writing)
pub const PICO_PAGE_SIZE: u32 = 0x100;
/// RP MCU flash sector size (for erasing)
pub const PICO_SECTOR_SIZE: u32 = 0x1000;
/// RP MCU memory address for the start of flash storage
pub const PICO_FLASH_START: u32 = 0x10000000;
/// RP MCU memory address for the initial stack pointer
pub const PICO_STACK_POINTER: u32 = 0x20042000; // same as SRAM_END_RP2040

/// RP USB Vendor ID
pub const PICOBOOT_VID: u16 = 0x2E8A;
/// RP2040 USB Product ID
pub const PICOBOOT_PID_RP2040: u16 = 0x0003;
/// RP2350 USB Product ID
pub const PICOBOOT_PID_RP2350: u16 = 0x000f;

/// RP MCU magic number for USB interfacing
pub const PICOBOOT_MAGIC: u32 = 0x431FD10B;

/// UF2 Family ID for RP2040
pub const UF2_RP2040_FAMILY_ID: u32 = 0xE48BFF56;
// pub const UF2_ABSOLUTE_FAMILY_ID: u32 = 0xE48BFF57;
// pub const UF2_DATA_FAMILY_ID: u32 = 0xE48BFF58;
/// UF2 Family ID for RP2350 (ARM, Secure TrustZone)
pub const UF2_RP2350_ARM_S_FAMILY_ID: u32 = 0xE48BFF59;
/// UF2 Family ID for RP2350 (RISC-V)
pub const UF2_RP2350_RISCV_FAMILY_ID: u32 = 0xE48BFF5A;
/// UF2 Family ID for RP2350 (ARM, Non-Secure TrustZone)
pub const UF2_RP2350_ARM_NS_FAMILY_ID: u32 = 0xE48BFF5B;
// pub const UF2_FAMILY_ID_MAX: u32 = 0xE48BFF5B;

/// Command Module
pub mod cmd;
pub use cmd::{PicobootCmd, PicobootCmdId, PicobootError, TargetID};

/// USB Connection Module
pub mod usb;
pub use usb::PicobootConnection;
