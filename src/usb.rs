use crate::{
    cmd::{PicobootCmd, PicobootError, PicobootStatusCmd, TargetID},
    PAGE_SIZE, PICOBOOT_PID_RP2040, PICOBOOT_PID_RP2350, PICOBOOT_VID, SECTOR_SIZE,
};

use bincode;
use rusb::{Device, DeviceDescriptor, DeviceHandle, Direction, TransferType, UsbContext};

// see https://github.com/raspberrypi/picotool/blob/master/main.cpp#L4173
// for loading firmware over a connection

type Error = PicobootError;
type Result<T> = ::std::result::Result<T, Error>;

/// A connection to a PICOBOOT device
///
/// This structure contains shorthand functions for send commands with checks to
/// ensure safety with use of PICOBOOT interface commands.
#[derive(Debug)]
pub struct PicobootConnection<T: UsbContext> {
    _context: T,
    _device: Device<T>,
    _desc: DeviceDescriptor,
    handle: DeviceHandle<T>,

    _cfg: u8,
    iface: u8,
    _setting: u8,
    in_addr: u8,
    out_addr: u8,

    cmd_token: u32,
    has_kernel_driver: bool,
    target_id: TargetID,
}
impl<T: UsbContext> Drop for PicobootConnection<T> {
    fn drop(&mut self) {
        self.handle
            .release_interface(self.iface)
            .expect("could not release interface");

        if self.has_kernel_driver {
            self.handle
                .attach_kernel_driver(self.iface)
                .expect("could not retach kernel driver");
        }
    }
}
impl<T: UsbContext> PicobootConnection<T> {
    /// Creates a new PICOBOOT connection
    ///
    /// Takes a rusb context and a USB VID/PID pair tuple. The VID/PID pair
    /// dictates how the connection determines the target. If `None` is
    /// provided, the connection attempts both RP2040 and RP2350 VID/PID pairs.
    /// If a VID/PID pair is provided, and if the pair belongs to the RP2040,
    /// the target will be considered an RP2040. Otherwise, the target will be
    /// considered an RP2350.
    ///
    /// # Errors
    /// - [`Error::UsbDeviceNotFound`]
    /// - [`Error::UsbEndpointsNotFound`]
    /// - [`Error::UsbEndpointsUnexpected`]
    /// - [`Error::UsbDetachKernelDriverFailure`]
    /// - [`Error::UsbClaimInterfaceFailure`]
    /// - [`Error::UsbSetAltSettingFailure`]
    ///
    /// # Panics
    /// - If the appropriate USB device is found but cannot be opened, this function will panic.
    pub fn new(mut ctx: T, vidpid: Option<(u16, u16)>) -> Result<Self> {
        // simple heuristic for determining target type
        let dev = match vidpid {
            Some((vid, pid)) => {
                let id = match (vid, pid) {
                    (PICOBOOT_VID, PICOBOOT_PID_RP2040) => TargetID::Rp2040,
                    _ => TargetID::Rp2350,
                };
                Self::open_device(&mut ctx, vid, pid).map(|d| (d, id))
            }
            None => [
                (PICOBOOT_VID, PICOBOOT_PID_RP2040, TargetID::Rp2040),
                (PICOBOOT_VID, PICOBOOT_PID_RP2350, TargetID::Rp2350),
            ]
            .into_iter()
            .find_map(|(vid, pid, id)| Self::open_device(&mut ctx, vid, pid).map(|d| (d, id))),
        };

        let Some(((device, desc, handle), target_id)) = dev else {
            return Err(Error::UsbDeviceNotFound);
        };

        let e1 = Self::get_endpoint(&device, 255, 0, 0, Direction::In, TransferType::Bulk);
        let e2 = Self::get_endpoint(&device, 255, 0, 0, Direction::Out, TransferType::Bulk);

        let (cfg, iface, setting, in_addr, out_addr) = match (e1, e2) {
            (None, _) | (_, None) => return Err(Error::UsbEndpointsNotFound),
            (Some((c1, i1, s1, in_addr)), Some((c2, i2, s2, out_addr))) => {
                if (c1, i1, s1) == (c2, i2, s2) {
                    (c2, i2, s2, in_addr, out_addr)
                } else {
                    return Err(Error::UsbEndpointsUnexpected);
                }
            }
        };

        let has_kernel_driver = if let Ok(true) = handle.kernel_driver_active(iface) {
            handle
                .detach_kernel_driver(iface)
                .map_err(Error::UsbDetachKernelDriverFailure)?;
            true
        } else {
            false
        };

        if handle.set_active_configuration(cfg).is_err() {
            // println!("Warning: could not set USB active configuration");
        }

        handle
            .claim_interface(iface)
            .map_err(Error::UsbClaimInterfaceFailure)?;
        handle
            .set_alternate_setting(iface, setting)
            .map_err(Error::UsbSetAltSettingFailure)?;

        Ok(PicobootConnection {
            _context: ctx,
            _device: device,
            _desc: desc,
            handle,

            _cfg: cfg,
            iface,
            _setting: setting,
            in_addr,
            out_addr,

            cmd_token: 1,
            has_kernel_driver,
            target_id,
        })
    }

    fn open_device(
        ctx: &mut T,
        vid: u16,
        pid: u16,
    ) -> Option<(Device<T>, DeviceDescriptor, DeviceHandle<T>)> {
        let devices = ctx.devices().ok()?;
        for device in devices.iter() {
            let device_desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue,
            };

            if device_desc.vendor_id() == vid && device_desc.product_id() == pid {
                match device.open() {
                    Ok(handle) => return Some((device, device_desc, handle)),
                    Err(e) => panic!("Device found but failed to open: {}", e),
                }
            }
        }

        None
    }

    fn get_endpoint(
        device: &Device<T>,
        class: u8,
        subclass: u8,
        protocol: u8,
        direction: Direction,
        transfer_type: TransferType,
    ) -> Option<(u8, u8, u8, u8)> {
        let desc = device.device_descriptor().unwrap();
        for n in 0..desc.num_configurations() {
            let config_desc = match device.config_descriptor(n) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for iface in config_desc.interfaces() {
                for iface_desc in iface.descriptors() {
                    let iface_class = iface_desc.class_code();
                    let iface_subclass = iface_desc.sub_class_code();
                    let iface_protocol = iface_desc.protocol_code();
                    if !(iface_class == class
                        && iface_subclass == subclass
                        && iface_protocol == protocol)
                    {
                        continue;
                    }

                    for endpoint_desc in iface_desc.endpoint_descriptors() {
                        if endpoint_desc.direction() == direction
                            && endpoint_desc.transfer_type() == transfer_type
                        {
                            return Some((
                                config_desc.number(),
                                iface_desc.interface_number(),
                                iface_desc.setting_number(),
                                endpoint_desc.address(),
                            ));
                        }
                    }
                }
            }
        }

        None
    }

    fn bulk_read(&mut self, buf_size: usize, check: bool) -> Result<Vec<u8>> {
        let mut buf = vec![0; buf_size];
        let timeout = std::time::Duration::from_secs(3);
        let len = self
            .handle
            .read_bulk(self.in_addr, &mut buf, timeout)
            .map_err(Error::UsbReadBulkFailure)?;

        if check && len != buf_size {
            return Err(Error::UsbReadBulkMismatch);
        }

        buf.resize(len, 0);
        Ok(buf)
    }

    fn bulk_write(&mut self, buf: &[u8], check: bool) -> Result<()> {
        let timeout = std::time::Duration::from_secs(5);
        let len = self
            .handle
            .write_bulk(self.out_addr, buf, timeout)
            .map_err(Error::UsbWriteBulkFailure)?;

        if check && len != buf.len() {
            return Err(Error::UsbWriteBulkMismatch);
        }

        Ok(())
    }

    /// Sends a command to the device
    ///
    /// Sends a command to the PICOBOOT device. Depending on the command, the
    /// buffer argument may be used to send data to the device. Depending on the
    /// command, the returned Vec will contain data from the device.
    ///
    /// # Errors
    /// - [`Error::CmdSerializeFailure`]
    /// - [`Error::UsbWriteBulkFailure`]
    /// - [`Error::UsbWriteBulkMismatch`]
    /// - [`Error::UsbReadBulkFailure`]
    /// - [`Error::UsbReadBulkMismatch`]
    pub fn cmd(&mut self, cmd: PicobootCmd, buf: &[u8]) -> Result<Vec<u8>> {
        let cmd = cmd.set_token(self.cmd_token);
        self.cmd_token += 1;

        // write command
        let cmdu8 = bincode::serialize(&cmd).map_err(Error::CmdSerializeFailure)?;
        self.bulk_write(cmdu8.as_slice(), true)?;
        let _stat = self.get_command_status();

        // if we're reading or writing a buffer
        let l = cmd.get_transfer_len().try_into().unwrap();
        let mut res = vec![];
        if l != 0 {
            if ((cmd.get_cmd_id() as u8) & 0x80) != 0 {
                res = self.bulk_read(l, true)?;
            } else {
                self.bulk_write(buf, true)?;
            }
            let _stat = self.get_command_status();
        }

        // do ack
        if ((cmd.get_cmd_id() as u8) & 0x80) != 0 {
            self.bulk_write(&[0u8; 1], false)?;
        } else {
            self.bulk_read(1, false)?;
        }

        Ok(res)
    }

    /// Requests non-exclusive access with the device, and does not close the
    /// USB Mass Storage interface.
    ///
    /// # Errors:
    /// - Any produced by [`Self::cmd`]
    pub fn access_not_exclusive(&mut self) -> Result<()> {
        self.set_exclusive_access(0)
    }

    /// Requests exclusive access with the device, and disables the USB Mass
    /// Storage interface. Any data writes through that interface will fail.
    ///
    /// # Errors:
    /// - Any produced by [`Self::cmd`]
    pub fn access_exclusive(&mut self) -> Result<()> {
        self.set_exclusive_access(1)
    }

    /// Requests exclusive access with the device, and disables and ejects the
    /// USB Mass Storage interface. Any data writes through that interface will
    /// fail.
    ///
    /// # Errors:
    /// - Any produced by [`Self::cmd`]
    pub fn access_exclusive_eject(&mut self) -> Result<()> {
        self.set_exclusive_access(2)
    }

    fn set_exclusive_access(&mut self, exclusive: u8) -> Result<()> {
        self.cmd(PicobootCmd::exclusive_access(exclusive), &[0u8; 0])?;
        Ok(())
    }

    /// Reboots the device with a specified program counter, stack pointer, and
    /// delay in milliseconds.
    ///
    /// - `pc` - Program counter to start the device with. Use `0` for a standard flash boot, or a RAM address to start executing at.
    /// - `sp` - Stack pointer to start the device with. Unused if `pc` is `0`.
    /// - `delay` - Time in milliseconds to start the device after.
    ///
    /// # Errors:
    /// - Any produced by [`Self::cmd`]
    pub fn reboot(&mut self, pc: u32, sp: u32, delay: u32) -> Result<()> {
        self.cmd(PicobootCmd::reboot(pc, sp, delay), &[0u8; 0])?;
        Ok(())
    }

    /// Reboots the device with a delay in milliseconds. (Only for RP2350)
    ///
    /// - `delay` - Time in milliseconds to start the device after.
    ///
    /// # Errors:
    /// - [`Error::CmdNotAllowedForTarget`]
    /// - Any produced by [`Self::cmd`]
    pub fn reboot2_normal(&mut self, delay: u32) -> Result<()> {
        if let TargetID::Rp2040 = self.target_id {
            return Err(Error::CmdNotAllowedForTarget);
        }

        self.cmd(PicobootCmd::reboot2_normal(delay), &[0u8; 0])?;
        Ok(())
    }

    /// Erases the flash memory of the device.
    ///
    /// - `addr` - Address to start the erase. Must be on a multiple of [`PICO_SECTOR_SIZE`].
    /// - `size` - Number of bytes to erase. Must be a multiple of [`PICO_SECTOR_SIZE`].
    ///
    /// # Errors:
    /// - [`Error::EraseInvalidAddr`]
    /// - [`Error::EraseInvalidSize`]
    /// - Any produced by [`Self::cmd`]
    pub fn flash_erase(&mut self, addr: u32, size: u32) -> Result<()> {
        if addr % SECTOR_SIZE != 0 {
            return Err(Error::EraseInvalidAddr);
        }
        if size % SECTOR_SIZE != 0 {
            return Err(Error::EraseInvalidSize);
        }

        self.cmd(PicobootCmd::flash_erase(addr, size), &[0u8; 0])?;
        Ok(())
    }

    /// Writes a buffer to the flash memory of the device.
    ///
    /// - `addr` - Address to start the write. Must be on a multiple of [`PICO_PAGE_SIZE`].
    /// - `buf` - Buffer of data to write to flash. Should be a multiple of [`PICO_PAGE_SIZE`]. If not, the remainder of the final page is zero-filled.
    ///
    /// # Errors:
    /// - [`Error::WriteInvalidAddr`]
    /// - Any produced by [`Self::cmd`]
    pub fn flash_write(&mut self, addr: u32, buf: &[u8]) -> Result<()> {
        if addr % PAGE_SIZE != 0 {
            return Err(Error::WriteInvalidAddr);
        }

        self.cmd(PicobootCmd::flash_write(addr, buf.len() as u32), buf)?;
        Ok(())
    }

    /// Writes a buffer to the flash memory of the device.
    ///
    /// - `addr` - Address to start the read.
    /// - `size` - Number of bytes to read.
    ///
    /// # Errors:
    /// - Any produced by [`Self::cmd`]
    pub fn flash_read(&mut self, addr: u32, size: u32) -> Result<Vec<u8>> {
        self.cmd(PicobootCmd::flash_read(addr, size), &[0u8; 0])
    }

    /// Enter Flash XIP (execute-in-place) mode.
    ///
    /// # Errors:
    /// - Any produced by [`Self::cmd`]
    pub fn enter_xip(&mut self) -> Result<()> {
        self.cmd(PicobootCmd::enter_xip(), &[0u8; 0])?;
        Ok(())
    }

    /// Exits Flash XIP (execute-in-place) mode.
    ///
    /// # Errors:
    /// - Any produced by [`Self::cmd`]
    pub fn exit_xip(&mut self) -> Result<()> {
        self.cmd(PicobootCmd::exit_xip(), &[0u8; 0])?;
        Ok(())
    }

    /// Resets PICOBOOT USB interface.
    ///
    /// This should be called after opening a brand new connection to ensure a
    /// consistent starting state.
    ///
    /// # Errors:
    /// - [`Error::UsbClearInAddrHalt`]
    /// - [`Error::UsbClearOutAddrHalt`]
    /// - [`Error::UsbResetInterfaceFailure`]
    pub fn reset_interface(&mut self) -> Result<()> {
        self.handle
            .clear_halt(self.in_addr)
            .map_err(Error::UsbClearInAddrHalt)?;
        self.handle
            .clear_halt(self.out_addr)
            .map_err(Error::UsbClearOutAddrHalt)?;

        let timeout = std::time::Duration::from_secs(1);
        self.handle
            .write_control(0x41, 0x41, 0, self.iface.into(), &[0u8; 0], timeout)
            .map_err(Error::UsbResetInterfaceFailure)?;

        Ok(())
    }

    fn get_command_status(&mut self) -> Result<PicobootStatusCmd> {
        let timeout = std::time::Duration::from_secs(1);
        let mut buf = [0u8; 16];
        let _res = self
            .handle
            .read_control(0xC1, 0x42, 0, self.iface.into(), &mut buf, timeout)
            .map_err(Error::UsbGetCommandStatusFailure)?;
        let buf = bincode::deserialize(&buf).map_err(Error::CmdDeserializeFailure)?;

        Ok(buf)
    }

    /// Returns PICOBOOT device type.
    ///
    /// Device type is determined by [`Self::new`].
    pub fn get_device_type(&self) -> TargetID {
        self.target_id
    }
}
