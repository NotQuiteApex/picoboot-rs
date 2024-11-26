use serde::{Deserialize, Serialize};

// see https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf
// section 2.8.5 for details on PICOBOOT interface

pub const PICO_PAGE_SIZE: usize = 256;
pub const PICO_SECTOR_SIZE: u32 = 4096;
pub const PICO_FLASH_START: u32 = 0x10000000;
pub const PICO_STACK_POINTER: u32 = 0x20042000;
pub const PICOBOOT_VID: u16 = 0x2E8A;
pub const PICOBOOT_PID_RP2040: u16 = 0x0003;
pub const PICOBOOT_PID_RP2350: u16 = 0x000f;
pub const PICOBOOT_MAGIC: u32 = 0x431FD10B;

#[derive(Debug, Clone, Copy)]
pub enum TargetID {
    Rp2040,
    Rp2350,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum PicobootCmdId {
    Unknown = 0x0,
    ExclusiveAccess = 0x1,
    Reboot = 0x2,
    FlashErase = 0x3,
    Read = 0x84, // either RAM or FLASH
    Write = 0x5, // either RAM or FLASH (does no erase)
    ExitXip = 0x6,
    EnterCmdXip = 0x7,
    Exec = 0x8,
    VectorizeFlash = 0x9,
    // RP2350 only below here
    Reboot2 = 0xA,
    GetInfo = 0x8B,
    OtpRead = 0x8C,
    OtpWrite = 0xD,
    //Exec2 = 0xE, // currently unused
}
impl TryFrom<u8> for PicobootCmdId {
    type Error = ();

    fn try_from(x: u8) -> Result<Self, Self::Error> {
        match x {
            x if x == Self::Unknown as u8 => Ok(Self::Unknown),
            x if x == Self::ExclusiveAccess as u8 => Ok(Self::ExclusiveAccess),
            x if x == Self::Reboot as u8 => Ok(Self::Reboot),
            x if x == Self::FlashErase as u8 => Ok(Self::FlashErase),
            x if x == Self::Read as u8 => Ok(Self::Read),
            x if x == Self::Write as u8 => Ok(Self::Write),
            x if x == Self::ExitXip as u8 => Ok(Self::ExitXip),
            x if x == Self::EnterCmdXip as u8 => Ok(Self::EnterCmdXip),
            x if x == Self::Exec as u8 => Ok(Self::Exec),
            x if x == Self::VectorizeFlash as u8 => Ok(Self::VectorizeFlash),
            x if x == Self::Reboot2 as u8 => Ok(Self::Reboot2),
            x if x == Self::GetInfo as u8 => Ok(Self::GetInfo),
            x if x == Self::OtpRead as u8 => Ok(Self::OtpRead),
            x if x == Self::OtpWrite as u8 => Ok(Self::OtpWrite),
            // x if x == Self::Exec2 as u8 => Ok(Self::Exec2),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum PicobootStatus {
    Ok = 0,
    UnknownCmd = 1,
    InvalidCmdLength = 2,
    InvalidTransferLength = 3,
    InvalidAddress = 4,
    BadAlignment = 5,
    InterleavedWrite = 6,
    Rebooting = 7,
    UnknownError = 8,
    InvalidState = 9,
    NotPermitted = 10,
    InvalidArg = 11,
    BufferTooSmall = 12,
    PreconditionNotMet = 13,
    ModifiedData = 14,
    InvalidData = 15,
    NotFound = 16,
    UnsupportedModification = 17,
}
impl TryFrom<u32> for PicobootStatus {
    type Error = ();

    fn try_from(x: u32) -> Result<Self, Self::Error> {
        match x {
            x if x == Self::Ok as u32 => Ok(Self::Ok),
            x if x == Self::UnknownCmd as u32 => Ok(Self::UnknownCmd),
            x if x == Self::InvalidCmdLength as u32 => Ok(Self::InvalidCmdLength),
            x if x == Self::InvalidTransferLength as u32 => Ok(Self::InvalidTransferLength),
            x if x == Self::InvalidAddress as u32 => Ok(Self::InvalidAddress),
            x if x == Self::BadAlignment as u32 => Ok(Self::BadAlignment),
            x if x == Self::InterleavedWrite as u32 => Ok(Self::InterleavedWrite),
            x if x == Self::Rebooting as u32 => Ok(Self::Rebooting),
            x if x == Self::UnknownError as u32 => Ok(Self::UnknownError),
            x if x == Self::InvalidState as u32 => Ok(Self::InvalidState),
            x if x == Self::NotPermitted as u32 => Ok(Self::NotPermitted),
            x if x == Self::InvalidArg as u32 => Ok(Self::InvalidArg),
            x if x == Self::BufferTooSmall as u32 => Ok(Self::BufferTooSmall),
            x if x == Self::PreconditionNotMet as u32 => Ok(Self::PreconditionNotMet),
            x if x == Self::ModifiedData as u32 => Ok(Self::ModifiedData),
            x if x == Self::InvalidData as u32 => Ok(Self::InvalidData),
            x if x == Self::NotFound as u32 => Ok(Self::NotFound),
            x if x == Self::UnsupportedModification as u32 => Ok(Self::UnsupportedModification),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[repr(C, packed)]
pub struct PicobootRangeCmd {
    addr: u32,
    size: u32,
    _unused: u64,
}
impl PicobootRangeCmd {
    pub fn ser(addr: u32, size: u32) -> [u8; 16] {
        let c = PicobootRangeCmd {
            addr: addr,
            size: size,
            _unused: 0,
        };
        bincode::serialize(&c)
            .unwrap()
            .try_into()
            .unwrap_or_else(|v: Vec<u8>| {
                panic!("Expected a Vec of length {} but it was {}", 16, v.len())
            })
    }
}

#[derive(Serialize, Debug, Clone)]
#[repr(C, packed)]
pub struct PicobootRebootCmd {
    pc: u32,
    sp: u32,
    delay: u32,
    _unused: u32,
}
impl PicobootRebootCmd {
    pub fn ser(pc: u32, sp: u32, delay: u32) -> [u8; 16] {
        let c = PicobootRebootCmd {
            pc: pc,
            sp: sp,
            delay: delay,
            _unused: 0,
        };
        bincode::serialize(&c)
            .unwrap()
            .try_into()
            .unwrap_or_else(|v: Vec<u8>| {
                panic!("Expected a Vec of length {} but it was {}", 16, v.len())
            })
    }
}

#[derive(Serialize, Debug, Clone)]
#[repr(C, packed)]
pub struct PicobootReboot2Cmd {
    flags: u32,
    delay: u32,
    p0: u32,
    p1: u32,
}
impl PicobootReboot2Cmd {
    pub fn ser(flags: u32, delay: u32, p0: u32, p1: u32) -> [u8; 16] {
        let c = PicobootReboot2Cmd {
            flags,
            delay,
            p0,
            p1,
        };
        bincode::serialize(&c)
            .unwrap()
            .try_into()
            .unwrap_or_else(|v: Vec<u8>| {
                panic!("Expected a Vec of length {} but it was {}", 16, v.len())
            })
    }
}

#[derive(Deserialize, Debug, Clone)]
#[repr(C, packed)]
pub struct PicobootStatusCmd {
    token: u32,
    status_code: u32,
    cmd_id: u8,
    in_progress: u8,
    _unused: [u8; 6],
}
impl PicobootStatusCmd {
	pub fn get_token(&self) -> u32 {
		self.token
	}

	pub fn get_status_code(&self) -> u32 {
		self.status_code
	}

	pub fn get_cmd_id(&self) -> u8 {
		self.cmd_id
	}

	pub fn get_in_progress(&self) -> u8 {
		self.in_progress
	}
}

#[derive(Serialize, Debug, Clone)]
#[repr(C, packed)]
pub struct PicobootCmd {
    magic: u32,
    token: u32,
    cmd_id: u8,
    cmd_size: u8,
    _unused: u16,
    transfer_len: u32,
    args: [u8; 16],
}
impl PicobootCmd {
    pub fn new(cmd_id: PicobootCmdId, cmd_size: u8, transfer_len: u32, args: [u8; 16]) -> Self {
        PicobootCmd {
            magic: PICOBOOT_MAGIC,
            token: 0,
            cmd_id: cmd_id as u8,
            cmd_size: cmd_size,
            _unused: 0,
            transfer_len: transfer_len,
            args: args,
        }
    }

	pub fn set_token(mut self, token: u32) -> Self {
		self.token = token;
		self
	}

	pub fn get_transfer_len(&self) -> u32 {
		self.transfer_len
	}

	pub fn get_cmd_id(&self) -> u8 {
		self.cmd_id
	}
}