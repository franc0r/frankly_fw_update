use crc::{Crc, CRC_32_ISO_HDLC};
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

// Flash Page -------------------------------------------------------------------------------------

pub struct FlashPage {
    id: u32,
    address: u32,
    bytes: Vec<u8>,
    crc: u32,
}

impl FlashPage {
    pub fn new(id: u32, address: u32, bytes: Vec<u8>) -> Self {
        FlashPage {
            id,
            address,
            bytes,
            crc: 0,
        }
    }

    pub fn calculate_crc(&mut self) {
        self.crc = CRC32.checksum(&self.bytes);
    }

    pub fn get_id(&self) -> u32 {
        self.id
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

    pub fn set_byte(&mut self, idx: usize, value: u8) {
        self.bytes[idx] = value;
    }

    pub fn get_byte_vec(&self) -> &Vec<u8> {
        &self.bytes
    }
}
