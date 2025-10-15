pub mod hex_file;

use crc::{Crc, CRC_32_ISO_HDLC};
use std::collections::HashMap;
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

use crate::francor::franklyboot::{
    flash::{FlashPage, FlashSection},
    Error,
};

// Firmware Data Trait ----------------------------------------------------------------------------

pub type FirmwareDataRaw = HashMap<u32, u8>;

pub trait FirmwareDataInterface {
    fn get_firmware_data(&self) -> Option<&FirmwareDataRaw>;
}

// Firmware ---------------------------------------------------------------------------------------

pub const FLASH_DFT_VALUE: u8 = 0xFF;

///
/// Application Firmware Data Representation
///
pub struct AppFirmware {
    /// Flash start address (important address where app area starts)
    app_start_address: u32,

    /// Flash page size
    flash_page_size: u32,

    /// Number of pages
    flash_num_pages: u32,

    /// Vector containing all pages of the firmware
    page_lst: Vec<FlashPage>,

    // CRC32 value of the complete firmware
    crc: u32,
}

impl AppFirmware {
    ///
    /// Create new empty firmware object
    ///
    pub fn new(app_start_address: u32, flash_page_size: u32, flash_num_pages: u32) -> Self {
        AppFirmware {
            app_start_address,
            flash_page_size,
            flash_num_pages,
            page_lst: Vec::new(),
            crc: 0,
        }
    }

    pub fn from_section(section: &FlashSection) -> Self {
        AppFirmware {
            app_start_address: section.get_address(),
            flash_page_size: section.get_page_size(),
            flash_num_pages: section.get_num_pages(),
            page_lst: Vec::new(),
            crc: 0,
        }
    }

    ///
    /// Append firmware data to the firmware object
    ///
    pub fn append_firmware(&mut self, data_raw: &FirmwareDataRaw) -> Result<(), Error> {
        // Sort hash map keys by rising order
        let mut byte_address_lst: Vec<u32> = data_raw.keys().copied().collect();
        byte_address_lst.sort();

        // Iterate over every byte
        for byte_address in byte_address_lst {
            // Check if byte address is valid otherwise throw error
            let byte_address_valid = byte_address >= self.app_start_address;
            if !byte_address_valid {
                return Err(Error::Error(format!("Firmware layout invalid! Byte address {:#010X} is out of range! (Min Address: {:#010X})", 
                byte_address, self.app_start_address)));
            }

            let page_id = (byte_address - self.app_start_address) / self.flash_page_size;
            let page_byte_idx = (byte_address - self.app_start_address) % self.flash_page_size;

            // Check if page ID is valid otherwise throw error
            let page_id_valid = page_id < self.flash_num_pages;
            if !page_id_valid {
                return Err(Error::Error(format!("Firmware layout invalid! Byte address {:#010X}, Page-ID {} is out of range! (Max ID: {})", 
                byte_address, page_id, self.flash_num_pages)));
            }

            // Check if flash page already exists, if not create a new one and get reference to it
            let page = match self._get_page_mut(page_id) {
                // Return existing page
                Some(e) => e,

                // Create a new page
                None => {
                    self.page_lst.push(FlashPage::new(
                        page_id,
                        self.app_start_address + page_id * self.flash_page_size,
                        vec![FLASH_DFT_VALUE; self.flash_page_size as usize],
                    ));

                    self._get_page_mut(page_id).unwrap()
                }
            };

            // Write byte to page
            page.set_byte(
                page_byte_idx as usize,
                *data_raw.get(&byte_address).unwrap(),
            );
        }

        // Calculate CRC for all pages
        for page in self.page_lst.iter_mut() {
            page.calculate_crc();
        }

        // Calculate CRC for complete app
        self._calc_app_crc();

        Ok(())
    }

    // Getters ------------------------------------------------------------------------------------

    ///
    /// Get CRC32 value of the complete firmware
    ///
    pub fn get_crc(&self) -> u32 {
        self.crc
    }

    ///
    /// Get reference to page
    ///
    pub fn get_page(&self, page_id: u32) -> Option<&FlashPage> {
        self.page_lst
            .iter()
            .find(|&page| page.get_id() == page_id)
            .map(|v| v as _)
    }

    ///
    /// Get reference to page list
    ///
    pub fn get_page_lst(&self) -> &Vec<FlashPage> {
        &self.page_lst
    }

    ///
    /// Get application start address
    ///
    pub fn get_app_start_address(&self) -> u32 {
        self.app_start_address
    }

    ///
    /// Get flash page size
    ///
    pub fn get_flash_page_size(&self) -> u32 {
        self.flash_page_size
    }

    ///
    /// Get flash num pages
    ///
    pub fn get_flash_num_pages(&self) -> u32 {
        self.flash_num_pages
    }

    // Private Functions --------------------------------------------------------------------------

    fn _get_page_mut(&mut self, page_id: u32) -> Option<&mut FlashPage> {
        self.page_lst
            .iter_mut()
            .find(|page| page.get_id() == page_id)
    }

    fn _calc_app_crc(&mut self) {
        // Create vector containing all bytes of the flash
        let mut app_flash = Vec::<u8>::new();
        for page_id in 0..self.flash_num_pages {
            // Check if page exists
            match self.get_page(page_id) {
                Some(page) => {
                    // Page exists append bytes to flash
                    for byte_value in page.get_bytes().iter() {
                        app_flash.push(*byte_value);
                    }
                }
                None => {
                    // Page does not exist fill bytes with default value
                    for _byte_idx in 0..self.flash_page_size {
                        app_flash.push(FLASH_DFT_VALUE);
                    }
                }
            }
        }

        // Last four bytes in flash are ignored, because they store the CRC value
        app_flash.pop();
        app_flash.pop();
        app_flash.pop();
        app_flash.pop();

        // Calculate app CRC
        self.crc = CRC32.checksum(&app_flash);
    }
}

// Tests ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_firmware_map_invalid_address() {
        let mut map: FirmwareDataRaw = HashMap::new();
        map.insert(0x07000000, 0x00);

        let mut app_fw = AppFirmware::new(0x08000000, 0x400, 0x10);
        let result = app_fw.append_firmware(&map);
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

        let mut app_fw = AppFirmware::new(0x08000000, 0x400, 0x10);
        let result = app_fw.append_firmware(&map);
        assert!(result.is_ok());

        assert_eq!(app_fw.get_page_lst().len(), 1);

        let page = app_fw.get_page(0).unwrap();
        assert_eq!(page.get_address(), 0x08000000);
        assert_eq!(page.get_byte_vec().len(), 0x400);
        assert_eq!(page.get_byte_vec()[0], 0x00);
        assert_eq!(page.get_byte_vec()[1], 0x01);
        assert_eq!(page.get_byte_vec()[2], 0x02);
        assert_eq!(page.get_byte_vec()[3], 0x03);
        assert_eq!(page.get_byte_vec()[4], 0xFF);
        assert_eq!(page.get_byte_vec()[5], 0x04);
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

        let mut app_fw = AppFirmware::new(0x08000000, 0x400, 0x10);
        let result = app_fw.append_firmware(&map);
        assert!(result.is_ok());

        assert_eq!(app_fw.get_page_lst().len(), 2);

        let page = app_fw.get_page(0).unwrap();
        assert_eq!(page.get_address(), 0x08000000);
        assert_eq!(page.get_byte_vec().len(), 0x400);
        assert_eq!(page.get_byte_vec()[0], 0x00);
        assert_eq!(page.get_byte_vec()[1], 0x01);
        assert_eq!(page.get_byte_vec()[2], 0x02);
        assert_eq!(page.get_byte_vec()[3], 0x03);
        assert_eq!(page.get_byte_vec()[4], 0xFF);
        assert_eq!(page.get_byte_vec()[5], 0x04);

        let page = app_fw.get_page(2).unwrap();
        assert_eq!(page.get_address(), 0x08000800);
        assert_eq!(page.get_byte_vec().len(), 0x400);
        assert_eq!(page.get_byte_vec()[0], 0x10);
        assert_eq!(page.get_byte_vec()[1], 0x11);
        assert_eq!(page.get_byte_vec()[2], 0xFF);
        assert_eq!(page.get_byte_vec()[3], 0xFF);
        assert_eq!(page.get_byte_vec()[4], 0xFF);
        assert_eq!(page.get_byte_vec()[5], 0xFF);
        assert_eq!(page.get_byte_vec()[6], 0xFF);
        assert_eq!(page.get_byte_vec()[7], 0xFF);
        assert_eq!(page.get_byte_vec()[8], 0xFF);
        assert_eq!(page.get_byte_vec()[9], 0xFF);
        assert_eq!(page.get_byte_vec()[10], 0xFF);
        assert_eq!(page.get_byte_vec()[11], 0xFF);
        assert_eq!(page.get_byte_vec()[12], 0xFF);
        assert_eq!(page.get_byte_vec()[13], 0xFF);
        assert_eq!(page.get_byte_vec()[14], 0xFF);
        assert_eq!(page.get_byte_vec()[15], 0x12);
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
