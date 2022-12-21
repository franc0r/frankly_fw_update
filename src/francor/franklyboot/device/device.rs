use crate::francor::franklyboot::{
    com::{
        msg::{MsgData, RequestType},
        ComError, ComInterface,
    },
    device,
};

use std::fmt;

use super::EntryType;

pub struct Device {
    entries: Vec<device::Entry>,
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BOOT_VER {:#03}.{:#03}.{:#03} | VID: {:#010X} | PID: {:#010X} |" ,

    }
}

impl Device {
    pub fn new() -> Self {
        let mut device = Self {
            entries: Vec::new(),
        };

        device._add_const_entry("Bootloader Version", RequestType::DevInfoBootloaderVersion);
        device._add_const_entry("Bootloader CRC", RequestType::DevInfoBootloaderCRC);
        device._add_const_entry("Vendor ID", RequestType::DevInfoVID);
        device._add_const_entry("Product ID", RequestType::DevInfoPID);
        device._add_const_entry("Production Date", RequestType::DevInfoPRD);
        device._add_const_entry("Unique ID", RequestType::DevInfoUID);

        device._add_const_entry("Flash Start Address", RequestType::FlashInfoStartAddr);
        device._add_const_entry("Flash Page Size", RequestType::FlashInfoPageSize);
        device._add_const_entry("Flash Number of Pages", RequestType::FlashInfoNumPages);

        device._add_const_entry("App First Page Index", RequestType::AppInfoPageIdx);
        device
    }

    pub fn init<T: ComInterface>(&mut self, interface: &mut T) -> Result<bool, ComError> {
        self._read_const_data(interface)
    }


    fn _add_const_entry(&mut self, name: &str, request_type: RequestType) {
        self.entries.push(device::Entry::new(
            device::EntryType::Const,
            name,
            request_type,
        ));
    }

    fn _read_const_data<T: ComInterface>(&mut self, interface: &mut T) -> Result<bool, ComError> {
        for entry in self.entries.iter_mut() {
            if entry.get_entry_type().is_const() {
                if entry.read_value(interface)?.is_none() {
                    return Ok(false);
                }
            }
        }

        return Ok(true);
    }

    fn _get_entry(&self, request_type: RequestType) -> &device::Entry {
        for entry in self.entries.iter_mut() {
            if *entry.get_request_type() == request_type {
                return entry;
            }
        }

        panic!("Entry \"{:#?}\" for request type not found in list!", request_type);
    }
}


// TODO -> Change timeout back to ERROR
// Retry is handled within com trait!
// If communication is not possible return error -> makes access easier