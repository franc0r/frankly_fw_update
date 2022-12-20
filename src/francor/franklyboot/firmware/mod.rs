pub mod hex_file;

use crc::{Crc, CRC_32_ISO_HDLC};
use std::collections::HashMap;
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

// Firmware Data Trait ----------------------------------------------------------------------------

pub type FirmwareDataRaw = HashMap<u32, u8>;

pub trait FirmwareDataInterface {
    fn get_firmware_data(&self) -> Option<&FirmwareDataRaw>;
}

// Flash Page -------------------------------------------------------------------------------------

pub struct FlashPage {
    address: u32,
    bytes: Vec<u8>,
    crc: u32,
}

impl FlashPage {
    pub fn from_firmware_data(
        firmware_data: &FirmwareDataRaw,
        flash_address: u32,
        page_size: u32,
        num_pages: u32,
    ) -> Result<HashMap<u32, FlashPage>, String> {
        let mut page_map: HashMap<u32, FlashPage> = HashMap::new();

        // Sort addresses by rising order and iterate over every byte
        let mut address_lst: Vec<u32> = firmware_data.keys().map(|x| *x).collect();

        address_lst.sort();
        for address in address_lst {
            // Check if address is valid
            let address_valid = address >= flash_address;
            if !address_valid {
                return Err(format!(
                    "Adress {:#X} is out of range! Flash starts at {:#X}!",
                    address, flash_address
                ));
            }

            let page_idx = (address - flash_address) / page_size;

            // Check if page is valid
            let page_idx_valid = page_idx < num_pages;
            if !page_idx_valid {
                return Err(format!(
                    "Page {} is out of range! Flash has only {} pages!",
                    page_idx, num_pages
                ));
            }

            let page_address = (address - flash_address) % page_size;

            // Create page if it does not exist
            if !page_map.contains_key(&page_idx) {
                page_map.insert(
                    page_idx,
                    FlashPage {
                        address: flash_address + page_idx * page_size,
                        bytes: vec![0xFF; page_size as usize],
                        crc: 0,
                    },
                );
            }

            // Insert byte into page
            let page = page_map.get_mut(&page_idx).unwrap();
            page.bytes[page_address as usize] = firmware_data[&address];
        }

        // Calculate CRC values
        for (_, page) in &mut page_map {
            page.calculate_crc();
        }

        Ok(page_map)
    }

    fn calculate_crc(&mut self) {
        self.crc = CRC32.checksum(&self.bytes);
    }

    pub fn get_address(&self) -> u32 {
        self.address
    }

    pub fn get_bytes(&self) -> &Vec<u8> {
        &self.bytes
    }

    pub fn get_crc(&self) -> u32 {
        self.crc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_firmware_map_invalid_address() {
        let mut map: FirmwareDataRaw = HashMap::new();
        map.insert(0x07000000, 0x00);

        let result = FlashPage::from_firmware_data(&map, 0x08000000, 0x400, 0x10);
        assert!(result.is_err());
    }

    #[test]
    fn from_firmware_map_one_page() {
        let mut map: FirmwareDataRaw = HashMap::new();
        map.insert(0x08000000, 0x00);
        map.insert(0x08000001, 0x01);
        map.insert(0x08000002, 0x02);
        map.insert(0x08000003, 0x03);
        map.insert(0x08000005, 0x04);

        let result = FlashPage::from_firmware_data(&map, 0x08000000, 0x400, 0x10);
        assert!(result.is_ok());

        let page_map = result.unwrap();
        assert_eq!(page_map.len(), 1);

        let page = page_map.get(&0).unwrap();
        assert_eq!(page.address, 0x08000000);
        assert_eq!(page.bytes.len(), 0x400);
        assert_eq!(page.bytes[0], 0x00);
        assert_eq!(page.bytes[1], 0x01);
        assert_eq!(page.bytes[2], 0x02);
        assert_eq!(page.bytes[3], 0x03);
        assert_eq!(page.bytes[4], 0xFF);
        assert_eq!(page.bytes[5], 0x04);
    }

    #[test]
    fn from_firmware_map_two_pages() {
        let mut map: FirmwareDataRaw = HashMap::new();
        map.insert(0x08000000, 0x00);
        map.insert(0x08000001, 0x01);
        map.insert(0x08000002, 0x02);
        map.insert(0x08000003, 0x03);
        map.insert(0x08000005, 0x04);

        map.insert(0x08000800, 0x10);
        map.insert(0x08000801, 0x11);
        map.insert(0x0800080F, 0x12);

        let result = FlashPage::from_firmware_data(&map, 0x08000000, 0x400, 0x10);
        assert!(result.is_ok());

        let page_map = result.unwrap();
        assert_eq!(page_map.len(), 2);

        let page = page_map.get(&0).unwrap();
        assert_eq!(page.address, 0x08000000);
        assert_eq!(page.bytes.len(), 0x400);
        assert_eq!(page.bytes[0], 0x00);
        assert_eq!(page.bytes[1], 0x01);
        assert_eq!(page.bytes[2], 0x02);
        assert_eq!(page.bytes[3], 0x03);
        assert_eq!(page.bytes[4], 0xFF);
        assert_eq!(page.bytes[5], 0x04);

        let page = page_map.get(&2).unwrap();
        assert_eq!(page.address, 0x08000800);
        assert_eq!(page.bytes.len(), 0x400);
        assert_eq!(page.bytes[0], 0x10);
        assert_eq!(page.bytes[1], 0x11);
        assert_eq!(page.bytes[2], 0xFF);
        assert_eq!(page.bytes[3], 0xFF);
        assert_eq!(page.bytes[4], 0xFF);
        assert_eq!(page.bytes[5], 0xFF);
        assert_eq!(page.bytes[6], 0xFF);
        assert_eq!(page.bytes[7], 0xFF);
        assert_eq!(page.bytes[8], 0xFF);
        assert_eq!(page.bytes[9], 0xFF);
        assert_eq!(page.bytes[10], 0xFF);
        assert_eq!(page.bytes[11], 0xFF);
        assert_eq!(page.bytes[12], 0xFF);
        assert_eq!(page.bytes[13], 0xFF);
        assert_eq!(page.bytes[14], 0xFF);
        assert_eq!(page.bytes[15], 0x12);
    }

    #[test]
    fn test_crc32_checksum_algo() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        let checksum = CRC32.checksum(&bytes);
        let checksum_exp = 0x40EFAB9E;

        println!("Calculated: {:#X}, Expected: {:#X}", checksum, checksum_exp);
        assert_eq!(checksum, checksum_exp);
    }
}
