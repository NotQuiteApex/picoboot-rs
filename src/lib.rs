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
//! drive to the computer. The PICOBOOT USB interface is (usually) also active
//! during this time, and can be used for more advanced management of the
//! microcontroller device.
//!

/// RP MCU memory address for the start of ROM storage
pub const ROM_START: u32 = 0x00000000;
/// RP2040 memory address for the end of ROM storage
pub const ROM_END_RP2040: u32 = 0x00004000;
/// RP2350 memory address for the end of ROM storage
pub const ROM_END_RP2350: u32 = 0x00008000;

/// RP MCU memory address for the start of flash storage
pub const FLASH_START: u32 = 0x10000000;
/// RP2040 memory address for the end of flash storage
pub const FLASH_END_RP2040: u32 = 0x11000000;
/// RP2350 memory address for the end of flash storage
pub const FLASH_END_RP2350: u32 = 0x12000000;

/// RP2040 memory address for the start of XIP (execute-in-place) SRAM storage
pub const XIP_SRAM_START_RP2040: u32 = 0x15000000;
/// RP2040 memory address for the end of XIP (execute-in-place) SRAM storage
pub const XIP_SRAM_END_RP2040: u32 = 0x15004000;
/// RP2350 memory address for the start of XIP (execute-in-place) SRAM storage
pub const XIP_SRAM_START_RP2350: u32 = 0x13ffc000;
/// RP2350 memory address for the end of XIP (execute-in-place) SRAM storage
pub const XIP_SRAM_END_RP2350: u32 = 0x14000000;

/// RP MCU memory address for the start of SRAM storage
pub const SRAM_START_RP2040: u32 = 0x20000000;
/// RP2040 memory address for the end of SRAM storage
pub const SRAM_END_RP2040: u32 = 0x20042000;
/// RP2350 memory address for the end of SRAM storage
pub const SRAM_END_RP2350: u32 = 0x20082000;

/// RP MCU flash page size (for writing)
pub const PAGE_SIZE: u32 = 0x100;
/// RP MCU flash sector size (for erasing)
pub const SECTOR_SIZE: u32 = 0x1000;
/// RP2040 memory address for the initial stack pointer
pub const STACK_POINTER_RP2040: u32 = 0x20042000; // same as SRAM_END_RP2040
/// RP2350 memory address for the initial stack pointer
pub const STACK_POINTER_RP2350: u32 = 0x20082000; // same as SRAM_END_RP2350

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
pub const UF2_ABSOLUTE_FAMILY_ID: u32 = 0xE48BFF57;
pub const UF2_DATA_FAMILY_ID: u32 = 0xE48BFF58;
/// UF2 Family ID for RP2350 (ARM, Secure TrustZone)
pub const UF2_RP2350_ARM_S_FAMILY_ID: u32 = 0xE48BFF59;
/// UF2 Family ID for RP2350 (RISC-V)
pub const UF2_RP2350_RISCV_FAMILY_ID: u32 = 0xE48BFF5A;
/// UF2 Family ID for RP2350 (ARM, Non-Secure TrustZone)
pub const UF2_RP2350_ARM_NS_FAMILY_ID: u32 = 0xE48BFF5B;
pub const UF2_FAMILY_ID_MAX: u32 = 0xE48BFF5B;

/// Command Module
pub mod cmd;
pub use cmd::{PicobootCmd, PicobootCmdId, PicobootError, TargetID};

/// USB Connection Module
pub mod usb;
pub use usb::PicobootConnection;
