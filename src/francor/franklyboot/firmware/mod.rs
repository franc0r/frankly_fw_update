pub mod hex_file;

mod flash_page;
pub use flash_page::FlashPage;

use crc::{Crc, CRC_32_ISO_HDLC};
use std::collections::HashMap;
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

use crate::francor::franklyboot::Error;

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
    /// Flash start address
    flash_start_address: u32,

    /// Flash app page id
    flash_app_page_id: u32,

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
    pub fn new(
        flash_start_address: u32,
        flash_app_page_id: u32,
        flash_page_size: u32,
        flash_num_pages: u32,
    ) -> Self {
        AppFirmware {
            flash_start_address: flash_start_address,
            flash_app_page_id: flash_app_page_id,
            flash_page_size: flash_page_size,
            flash_num_pages: flash_num_pages,
            page_lst: Vec::new(),
            crc: 0,
        }
    }

    ///
    /// Append firmware data to the firmware object
    ///
    pub fn append_firmware(&mut self, data_raw: &FirmwareDataRaw) -> Result<(), Error> {
        // Sort hash map keys by rising order
        let mut byte_address_lst: Vec<u32> = data_raw.keys().map(|x| *x).collect();
        byte_address_lst.sort();

        // Iterate over every byte
        for byte_address in byte_address_lst {
            let page_id = (byte_address - self.flash_start_address) / self.flash_page_size;
            let page_byte_idx = (byte_address - self.flash_start_address) % self.flash_page_size;

            // Check if page ID is valid otherwise throw error
            let page_id_valid = page_id >= self.flash_app_page_id && page_id < self.flash_num_pages;
            if !page_id_valid {
                return Err(Error::Error(format!("Firmware layout invalid! Byte address {:#010X}, Page-ID {} is out of range! (Min ID: {} Max ID: {})", 
                byte_address, page_id, self.flash_app_page_id, self.flash_num_pages)));
            }

            // Check if flash page already exists, if not create a new one and get reference to it
            let page = match self._get_page_mut(page_id) {
                // Return existing page
                Some(e) => e,

                // Create a new page
                None => {
                    self.page_lst.push(FlashPage::new(
                        page_id,
                        self.flash_start_address + page_id * self.flash_page_size,
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

    // Private Functions --------------------------------------------------------------------------

    fn _get_page_mut(&mut self, page_id: u32) -> Option<&mut FlashPage> {
        for page in self.page_lst.iter_mut() {
            if page.get_id() == page_id {
                return Some(page);
            }
        }

        return None;
    }

    fn _get_page(&self, page_id: u32) -> Option<&FlashPage> {
        for page in self.page_lst.iter() {
            if page.get_id() == page_id {
                return Some(page);
            }
        }

        return None;
    }

    fn _calc_app_crc(&mut self) {
        // Create vector containing all bytes of the flash
        let mut app_flash = Vec::<u8>::new();
        for page_id in self.flash_app_page_id..self.flash_num_pages {
            // Check if page exists
            match self._get_page(page_id) {
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

pub struct FlashPageList {
    page_vec: Vec<FlashPage>,
}

impl FlashPageList {
    pub fn new() -> FlashPageList {
        FlashPageList {
            page_vec: Vec::new(),
        }
    }

    pub fn from_firmware_data(
        firmware_data: &FirmwareDataRaw,
        flash_address: u32,
        page_size: u32,
        num_pages: u32,
    ) -> Result<FlashPageList, Error> {
        // Create new page
        let mut page_lst = FlashPageList::new();

        // Sort addresses by rising order and iterate over every byte
        let mut address_lst: Vec<u32> = firmware_data.keys().map(|x| *x).collect();

        address_lst.sort();
        for address in address_lst {
            // Check if address is valid
            let address_valid = address >= flash_address;
            if !address_valid {
                return Err(Error::Error(format!(
                    "Adress {:#X} is out of range! Flash starts at {:#X}!",
                    address, flash_address
                )));
            }

            let page_idx = (address - flash_address) / page_size;

            // Check if page is valid
            let page_idx_valid = page_idx < num_pages;
            if !page_idx_valid {
                return Err(Error::Error(format!(
                    "Page {} is out of range! Flash has only {} pages!",
                    page_idx, num_pages
                )));
            }

            let page_address = (address - flash_address) % page_size;

            // Check if page entry exists if not create one
            let page = match page_lst.get_mut(page_idx) {
                Some(e) => e,
                None => {
                    page_lst.push(FlashPage::new(
                        page_idx,
                        flash_address + page_idx * page_size,
                        vec![FLASH_DFT_VALUE; page_size as usize],
                    ));

                    page_lst.get_mut(page_idx).unwrap()
                }
            };

            page.set_byte(page_address as usize, firmware_data[&address]);
        }

        // Calculate CRC values
        for page in page_lst.get_vec_mut().iter_mut() {
            page.calculate_crc()
        }

        Ok(page_lst)
    }

    pub fn get(&self, id: u32) -> Option<&FlashPage> {
        for page in self.page_vec.iter() {
            if page.get_id() == id {
                return Some(page);
            }
        }

        return None;
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut FlashPage> {
        for page in self.page_vec.iter_mut() {
            if page.get_id() == id {
                return Some(page);
            }
        }

        return None;
    }

    pub fn push(&mut self, page: FlashPage) {
        self.page_vec.push(page);
    }

    pub fn get_vec(&self) -> &Vec<FlashPage> {
        &self.page_vec
    }

    pub fn get_vec_mut(&mut self) -> &mut Vec<FlashPage> {
        &mut self.page_vec
    }

    pub fn len(&self) -> usize {
        self.page_vec.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_firmware_map_invalid_address() {
        let mut map: FirmwareDataRaw = HashMap::new();
        map.insert(0x07000000, 0x00);

        let result = FlashPageList::from_firmware_data(&map, 0x08000000, 0x400, 0x10);
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

        let result = FlashPageList::from_firmware_data(&map, 0x08000000, 0x400, 0x10);
        assert!(result.is_ok());

        let page_map = result.unwrap();
        assert_eq!(page_map.len(), 1);

        let page = page_map.get(0).unwrap();
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

        let result = FlashPageList::from_firmware_data(&map, 0x08000000, 0x400, 0x10);
        assert!(result.is_ok());

        let page_map = result.unwrap();
        assert_eq!(page_map.len(), 2);

        let page = page_map.get(0).unwrap();
        assert_eq!(page.get_address(), 0x08000000);
        assert_eq!(page.get_byte_vec().len(), 0x400);
        assert_eq!(page.get_byte_vec()[0], 0x00);
        assert_eq!(page.get_byte_vec()[1], 0x01);
        assert_eq!(page.get_byte_vec()[2], 0x02);
        assert_eq!(page.get_byte_vec()[3], 0x03);
        assert_eq!(page.get_byte_vec()[4], 0xFF);
        assert_eq!(page.get_byte_vec()[5], 0x04);

        let page = page_map.get(2).unwrap();
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
