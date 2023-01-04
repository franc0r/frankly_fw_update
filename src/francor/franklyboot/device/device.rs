use crate::francor::franklyboot::{
    com::{msg::RequestType, ComInterface},
    device,
    firmware::FirmwareDataInterface,
    Error,
};

// Device -----------------------------------------------------------------------------------------

///
/// Device Representationg
///
/// This struct represents the device. It contains all information about the device and provides
/// functions to read and write data from and to the device.
///
pub struct Device {
    entries: Vec<device::Entry>,
}

/// Implementation of the Display trait for the Device struct
//impl fmt::Display for Device {
//    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//        write!(f, "BOOT_VER {:#03}.{:#03}.{:#03} | VID: {:#010X} | PID: {:#010X} |" ,
//
//    }
//}

impl Device {
    /// Create a new device
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

    /// Initialize the device struct
    ///
    /// This function reads all constant data from the device and stores it in the device struct.
    pub fn init<T: ComInterface>(&mut self, interface: &mut T) -> Result<(), Error> {
        self._read_const_data(interface)
    }

    pub fn flash<T: ComInterface, FWI: FirmwareDataInterface>(
        &mut self,
        _interface: &mut T,
        fwi: &FWI,
    ) -> Result<(), Error> {
        // Read necessary data to variables
        let _flash_start = self.get_entry_value(RequestType::FlashInfoStartAddr);
        let flash_page_size = self.get_entry_value(RequestType::FlashInfoPageSize);
        let _flash_num_pages = self.get_entry_value(RequestType::FlashInfoNumPages);
        let _flash_app_page_idx = self.get_entry_value(RequestType::AppInfoPageIdx);
        let firmware_size = fwi.get_firmware_data().unwrap().len() as u32;
        let firmware_num_pages = (firmware_size / flash_page_size) + 1;

        println!(
            "Firmware Data: Size: {:#.2} kB Num Pages: {}",
            (firmware_size as f32 / 1024.0),
            firmware_num_pages
        );

        Ok(())
    }

    // Getters ------------------------------------------------------------------------------------

    /// Get entry of request type
    ///
    /// This function returns the entry of the given request type. If no entry is found, None is
    /// returned.
    ///
    fn get_entry(&self, request_type: RequestType) -> Option<&device::Entry> {
        for entry in self.entries.iter() {
            if *entry.get_request_type() == request_type {
                return Some(entry);
            }
        }

        return None;
    }

    /// Get entry value of request type
    ///
    /// This function returns the value of the entry of the given request type. If no entry is found,
    /// the function panics.
    pub fn get_entry_value(&self, request_type: RequestType) -> u32 {
        self.get_entry(request_type)
            .unwrap()
            .get_value()
            .unwrap()
            .to_word()
    }

    // Private Functions --------------------------------------------------------------------------

    fn _add_const_entry(&mut self, name: &str, request_type: RequestType) {
        self.entries.push(device::Entry::new(
            device::EntryType::Const,
            name,
            request_type,
        ));
    }

    fn _read_const_data<T: ComInterface>(&mut self, interface: &mut T) -> Result<(), Error> {
        for entry in self.entries.iter_mut() {
            if entry.get_entry_type().is_const() {
                entry.read_value(interface)?;
            }
        }

        return Ok(());
    }
}

// TODO -> Change timeout back to ERROR
// Retry is handled within com trait!
// If communication is not possible return error -> makes access easier

// Tests ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::francor::franklyboot::{
        com::{
            msg::{Msg, MsgData, ResultType},
            ComSimulator,
        },
        firmware::hex_file::HexFile,
    };

    #[test]
    fn device_flash() {
        let firmware = HexFile::from_file("./tests/data/TestFirmware.hex").unwrap();
        let mut device = Device::new();
        let mut com = new_com_sim_with_data();

        device.init(&mut com).unwrap();

        device.flash(&mut com, &firmware).unwrap();
    }

    #[test]
    fn device_new_and_init() {
        let mut com = new_com_sim_with_data();
        let mut device = Device::new();
        device.init(&mut com).unwrap();

        assert_eq!(
            device.get_entry_value(RequestType::DevInfoBootloaderVersion),
            0x00030201
        );

        assert_eq!(
            device.get_entry_value(RequestType::DevInfoBootloaderCRC),
            0xDEADBEEF
        );

        assert_eq!(device.get_entry_value(RequestType::DevInfoVID), 1);
        assert_eq!(device.get_entry_value(RequestType::DevInfoPID), 2);
        assert_eq!(device.get_entry_value(RequestType::DevInfoPRD), 3);
        assert_eq!(device.get_entry_value(RequestType::DevInfoUID), 4);

        assert_eq!(
            device.get_entry_value(RequestType::FlashInfoStartAddr),
            0x08000000
        );
        assert_eq!(
            device.get_entry_value(RequestType::FlashInfoPageSize),
            0x0400
        );
        assert_eq!(
            device.get_entry_value(RequestType::FlashInfoNumPages),
            0x000F
        );
        assert_eq!(device.get_entry_value(RequestType::AppInfoPageIdx), 0x0002);
    }

    #[test]
    fn device_new_no_init_get_entry() {
        let device = Device::new();

        assert!(device
            .get_entry(RequestType::DevInfoBootloaderVersion)
            .unwrap()
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoBootloaderCRC)
            .unwrap()
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoVID)
            .unwrap()
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoPID)
            .unwrap()
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoPRD)
            .unwrap()
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoUID)
            .unwrap()
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::FlashInfoStartAddr)
            .unwrap()
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::FlashInfoPageSize)
            .unwrap()
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::FlashInfoNumPages)
            .unwrap()
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::AppInfoPageIdx)
            .unwrap()
            .get_value()
            .is_none());
    }

    #[test]
    #[should_panic]
    fn deview_new_not_init_get_value() {
        let device = Device::new();
        device.get_entry_value(RequestType::DevInfoBootloaderVersion);
    }

    // Helpers ------------------------------------------------------------------------------------

    fn new_com_sim_with_data() -> ComSimulator {
        let mut interface = ComSimulator::new();

        interface.add_response(Msg::new(
            RequestType::DevInfoBootloaderVersion,
            ResultType::Ok,
            0,
            &MsgData::from_word(0x00030201),
        ));

        interface.add_response(Msg::new(
            RequestType::DevInfoBootloaderCRC,
            ResultType::Ok,
            0,
            &MsgData::from_word(0xDEADBEEF),
        ));

        interface.add_response(Msg::new(
            RequestType::DevInfoVID,
            ResultType::Ok,
            0,
            &MsgData::from_word(0x00000001),
        ));

        interface.add_response(Msg::new(
            RequestType::DevInfoPID,
            ResultType::Ok,
            0,
            &MsgData::from_word(0x00000002),
        ));

        interface.add_response(Msg::new(
            RequestType::DevInfoPRD,
            ResultType::Ok,
            0,
            &MsgData::from_word(0x00000003),
        ));

        interface.add_response(Msg::new(
            RequestType::DevInfoUID,
            ResultType::Ok,
            0,
            &MsgData::from_word(0x00000004),
        ));

        interface.add_response(Msg::new(
            RequestType::FlashInfoStartAddr,
            ResultType::Ok,
            0,
            &MsgData::from_word(0x08000000),
        ));

        interface.add_response(Msg::new(
            RequestType::FlashInfoPageSize,
            ResultType::Ok,
            0,
            &MsgData::from_word(0x00000400),
        ));

        interface.add_response(Msg::new(
            RequestType::FlashInfoNumPages,
            ResultType::Ok,
            0,
            &MsgData::from_word(0x0000000F),
        ));

        interface.add_response(Msg::new(
            RequestType::AppInfoPageIdx,
            ResultType::Ok,
            0,
            &MsgData::from_word(0x00000002),
        ));

        interface
    }
}
