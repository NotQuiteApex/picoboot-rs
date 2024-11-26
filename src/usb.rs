
use crate::cmd::{PicobootCmd, PicobootCmdId, PicobootRangeCmd, PicobootReboot2Cmd, PicobootRebootCmd, PicobootStatus, PicobootStatusCmd, TargetID, PICOBOOT_PID_RP2040, PICOBOOT_PID_RP2350, PICOBOOT_VID};

use bincode;
use rusb::{Device, DeviceDescriptor, DeviceHandle, Direction, TransferType, UsbContext};


// see https://github.com/raspberrypi/picotool/blob/master/main.cpp#L4173
// for loading firmware over a connection

#[derive(Debug)]
pub struct PicobootConnection<T: UsbContext> {
    context: T,
    device: Device<T>,
    desc: DeviceDescriptor,
    handle: DeviceHandle<T>,

    cfg: u8,
    iface: u8,
    setting: u8,
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
                .expect("could not retach kernel driver")
        }
    }
}
impl<T: UsbContext> PicobootConnection<T> {
    pub fn new(mut ctx: T, vidpid: impl Into<Option<(u16, u16)>>) -> Self {
        let (device, target_id) = match vidpid.into() {
            Some((vid, pid)) => {
                let target_id = if vid == PICOBOOT_VID && pid == PICOBOOT_PID_RP2040 {
                    TargetID::Rp2040
                } else {
                    TargetID::Rp2350
                };

                if let Some(device) = Self::open_device(&mut ctx, vid, pid) {
                    (Some(device), Some(target_id))
                } else {
                    (None, None)
                }
            }
            None => {
                if let Some(device) = Self::open_device(&mut ctx, PICOBOOT_VID, PICOBOOT_PID_RP2040) {
                    (Some(device), Some(TargetID::Rp2040))
                } else {
                    if let Some(device) = Self::open_device(&mut ctx, PICOBOOT_VID, PICOBOOT_PID_RP2350) {
                        (Some(device), Some(TargetID::Rp2350))
                    } else {
                        (None, None)
                    }
                }
            }
        };

        match device {
            Some((device, desc, handle)) => {
                let (_cfg, _iface, _setting, in_addr) =
                    Self::get_endpoint(&device, 0xFF, 0, 0, Direction::In, TransferType::Bulk)
                        .unwrap();
                let (cfg, iface, setting, out_addr) =
                    Self::get_endpoint(&device, 0xFF, 0, 0, Direction::Out, TransferType::Bulk)
                        .unwrap();

                if _cfg != cfg || _iface != iface || _setting != setting {
                    panic!("something doesnt match with the endpoints! {} != {} || {} != {} || {} != {}", _cfg, cfg, _iface, iface, _setting, setting)
                }

                let has_kernel_driver = match handle.kernel_driver_active(iface) {
                    Ok(true) => {
                        handle
                            .detach_kernel_driver(iface)
                            .expect("could not detach kernel driver");
                        true
                    }
                    _ => false,
                };

                if !handle.set_active_configuration(cfg).is_ok() {
                    println!("Warning: could not set USB active configuration");
                }
                handle
                    .claim_interface(iface)
                    .expect("could not claim interface");
                handle
                    .set_alternate_setting(iface, setting)
                    .expect("could not set alt setting");

                return PicobootConnection {
                    context: ctx,
                    device: device,
                    desc: desc,
                    handle: handle,

                    cfg: cfg,
                    iface: iface,
                    setting: setting,
                    in_addr: in_addr,
                    out_addr: out_addr,

                    cmd_token: 1,
                    has_kernel_driver: has_kernel_driver,
                    target_id: target_id.unwrap(),
                };
            }
            None => panic!("Could not find picoboot device."),
        }
    }
    
    fn open_device(
        ctx: &mut T,
        vid: u16,
        pid: u16,
    ) -> Option<(Device<T>, DeviceDescriptor, DeviceHandle<T>)> {
        let devices = match ctx.devices() {
            Ok(d) => d,
            Err(_) => return None,
        };
    
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

        return None;
    }

    fn bulk_read(&mut self, buf_size: usize, check: bool) -> rusb::Result<Vec<u8>> {
        let mut buf: Vec<u8> = vec![0; buf_size]; // [0; SECTOR_SIZE];
        let timeout = std::time::Duration::from_secs(3);
        let len = self
            .handle
            .read_bulk(self.in_addr, &mut buf, timeout)
            .expect("read_bulk failed");

        if check && len != buf_size {
            panic!("read mismatch {} != {}", len, buf_size)
        }

        buf.resize(len, 0);
        Ok(buf)
    }

    fn bulk_write(&mut self, mut buf: Vec<u8>, check: bool) -> rusb::Result<()> {
        let timeout = std::time::Duration::from_secs(5);
        let len = self
            .handle
            .write_bulk(self.out_addr, &mut buf, timeout)
            .expect("write_bulk failed");

        if check && len != buf.len() {
            panic!("write mismatch {} != {}", len, buf.len())
        }

        Ok(())
    }

    fn cmd(&mut self, cmd: PicobootCmd, buf: Vec<u8>) -> rusb::Result<Vec<u8>> {
        let cmd = cmd.set_token(self.cmd_token);
        self.cmd_token = self.cmd_token + 1;

        // write command
        let cmdu8 = bincode::serialize(&cmd).expect("failed to serialize cmd");
        self.bulk_write(cmdu8, true).expect("failed to write cmd");
        let _stat = self.get_command_status();

        // if we're reading or writing a buffer
        let l = cmd.get_transfer_len().try_into().unwrap();
        let mut res: Option<Vec<_>> = Some(vec![]);
        if l != 0 {
            if (cmd.get_cmd_id() & 0x80) != 0 {
                res = Some(self.bulk_read(l, true).unwrap());
            } else {
                self.bulk_write(buf, true).unwrap()
            }
            let _stat = self.get_command_status();
        }

        // do ack
        if (cmd.get_cmd_id() & 0x80) != 0 {
            self.bulk_write(vec![0], false).unwrap();
        } else {
            self.bulk_read(1, false).unwrap();
        }

        Ok(res.unwrap())
    }

    pub fn access_not_exclusive(&mut self) -> rusb::Result<()> {
        self.set_exclusive_access(0)
    }

    pub fn access_exclusive(&mut self) -> rusb::Result<()> {
        self.set_exclusive_access(1)
    }

    pub fn access_exclusive_eject(&mut self) -> rusb::Result<()> {
        self.set_exclusive_access(2)
    }

    fn set_exclusive_access(&mut self, exclusive: u8) -> rusb::Result<()> {
        let mut args = [0; 16];
        args[0] = exclusive;
        let cmd = PicobootCmd::new(PicobootCmdId::ExclusiveAccess, 1, 0, args);
        Ok(self.cmd(cmd, vec![]).map(|_| ())?)
    }

    pub fn reboot(&mut self, pc: u32, sp: u32, delay: u32) -> rusb::Result<()> {
        let args = PicobootRebootCmd::ser(pc, sp, delay);
        let cmd = PicobootCmd::new(PicobootCmdId::Reboot, 12, 0, args);
        Ok(self.cmd(cmd, vec![]).map(|_| ())?)
    }

    pub fn reboot2_normal(&mut self, delay: u32) -> rusb::Result<()> {
        let flags: u32 = 0x0; // Normal boot
        let args = PicobootReboot2Cmd::ser(flags, delay, 0, 0);
        let cmd = PicobootCmd::new(PicobootCmdId::Reboot2, 0x10, 0, args);
        Ok(self.cmd(cmd, vec![]).map(|_| ())?)
    }

    pub fn flash_erase(&mut self, addr: u32, size: u32) -> rusb::Result<()> {
        let args = PicobootRangeCmd::ser(addr, size);
        let cmd = PicobootCmd::new(PicobootCmdId::FlashErase, 8, 0, args);
        Ok(self.cmd(cmd, vec![]).map(|_| ())?)
    }

    pub fn flash_write(&mut self, addr: u32, buf: Vec<u8>) -> rusb::Result<()> {
        let args = PicobootRangeCmd::ser(addr, buf.len() as u32);
        let cmd = PicobootCmd::new(PicobootCmdId::Write, 8, buf.len() as u32, args);
        Ok(self.cmd(cmd, buf).map(|_| ())?)
    }

    pub fn flash_read(&mut self, addr: u32, size: u32) -> rusb::Result<Vec<u8>> {
        let args = PicobootRangeCmd::ser(addr, size);
        let cmd = PicobootCmd::new(PicobootCmdId::Read, 8, size, args);
        self.cmd(cmd, vec![])
    }

    pub fn enter_xip(&mut self) -> rusb::Result<()> {
        let args = [0; 16];
        let cmd = PicobootCmd::new(PicobootCmdId::EnterCmdXip, 0, 0, args);
        Ok(self.cmd(cmd, vec![]).map(|_| ())?)
    }

    pub fn exit_xip(&mut self) -> rusb::Result<()> {
        let args = [0; 16];
        let cmd = PicobootCmd::new(PicobootCmdId::ExitXip, 0, 0, args);
        Ok(self.cmd(cmd, vec![]).map(|_| ())?)
    }

    pub fn reset_interface(&mut self) {
        self.handle
            .clear_halt(self.in_addr)
            .expect("failed to clear in addr halt");
        self.handle
            .clear_halt(self.out_addr)
            .expect("failed to clear out addr halt");

        let timeout = std::time::Duration::from_secs(1);
        let mut buf = [0u8; 0];
        let _res = self
            .handle
            .write_control(
                0b01000001,
                0b01000001,
                0,
                self.iface.into(),
                &mut buf,
                timeout,
            )
            .expect("failed to reset interface");
    }

    fn get_command_status(&mut self) -> PicobootStatusCmd {
        let timeout = std::time::Duration::from_secs(1);
        let mut buf = [0u8; 16];
        let _res = self
            .handle
            .read_control(
                0b11000001,
                0b01000010,
                0,
                self.iface.into(),
                &mut buf,
                timeout,
            )
            .expect("failed to get command status");
        let buf: PicobootStatusCmd =
            bincode::deserialize(&buf).expect("failed to parse command status buffer");

        let tkn = buf.get_token();
        let stat = buf.get_status_code();
        let cmdid = buf.get_cmd_id();
        let wip = buf.get_in_progress();
        println!(
            "\t\tcmdstat => tkn={}, stat={:?}, cmdid={:?}, wip={}",
            tkn,
            PicobootStatus::try_from(stat).unwrap(),
            PicobootCmdId::try_from(cmdid).unwrap(),
            wip == 1
        );

        buf
    }

    pub fn get_device_type(&self) -> TargetID {
        self.target_id
    }
}