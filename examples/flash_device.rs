// Flashes UF2 to a Pico 1

use picoboot_rs::{
    PicobootConnection, TargetID, FLASH_START, PAGE_SIZE, SECTOR_SIZE, STACK_POINTER_RP2040,
};

use rusb::Context;
use uf2_decode::convert_from_uf2;

// creates a vector of vectors of u8's that map to flash pages sequentially
fn uf2_pages(bytes: Vec<u8>) -> Vec<Vec<u8>> {
    // loads the uf2 file into a binary
    let fw = convert_from_uf2(&bytes).expect("failed to parse uf2").0;

    let mut fw_pages: Vec<Vec<u8>> = vec![];
    let len = fw.len();

    // splits the binary into sequential pages
    for i in (0..len).step_by(PAGE_SIZE as usize) {
        let size = std::cmp::min(len - i, PAGE_SIZE as usize);
        let mut page = fw[i..i + size].to_vec();
        page.resize(PAGE_SIZE as usize, 0);
        fw_pages.push(page);
    }

    fw_pages
}

fn main() {
    match Context::new() {
        Ok(ctx) => {
            // create connection object
            let mut conn = PicobootConnection::new(ctx, None)
                .expect("failed to connect to PICOBOOT interface");

            conn.reset_interface().expect("failed to reset interface");
            conn.access_exclusive_eject()
                .expect("failed to claim access");
            conn.exit_xip().expect("failed to exit from xip mode");

            // firmware in a big vector of u8's
            let fw = std::fs::read("blink.uf2").expect("failed to read firmware");
            let fw_pages = uf2_pages(fw);

            // erase space on flash
            for (i, _) in fw_pages.iter().enumerate() {
                let addr = (i as u32) * PAGE_SIZE + FLASH_START;
                if (addr % SECTOR_SIZE) == 0 {
                    conn.flash_erase(addr, SECTOR_SIZE)
                        .expect("failed to erase flash");
                }
            }

            for (i, page) in fw_pages.iter().enumerate() {
                let addr = (i as u32) * PAGE_SIZE + FLASH_START;
                let size = PAGE_SIZE as u32;

                // write page to flash
                conn.flash_write(addr, page).expect("failed to write flash");

                // confirm flash write was successful
                let read = conn.flash_read(addr, size).expect("failed to read flash");
                let matching = page.iter().zip(&read).all(|(&a, &b)| a == b);
                assert!(matching, "page does not match flash");
            }

            // reboot device to start firmware
            let delay = 500; // in milliseconds
            match conn.get_device_type() {
                TargetID::Rp2040 => {
                    conn.reboot(0x0, STACK_POINTER_RP2040, delay)
                        .expect("failed to reboot device");
                }
                TargetID::Rp2350 => conn.reboot2_normal(delay).expect("failed to reboot device"),
            }
        }
        Err(e) => panic!("Could not initialize libusb: {}", e),
    }
}
