use std::collections::HashMap;

pub mod hex_file;

pub type FirmwareDataRaw = HashMap<u32, u8>;

pub trait FirmwareDataInterface {
    fn get_firmware_data(&self) -> Option<&FirmwareDataRaw>;
}
