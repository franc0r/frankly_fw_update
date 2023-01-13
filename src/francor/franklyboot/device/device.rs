use crate::francor::franklyboot::{
    com::{msg::MsgData, msg::RequestType, ComInterface},
    device::{Entry, EntryType},
    firmware::{AppFirmware, FirmwareDataInterface},
    flash::FlashDesc,
    Error,
};
use std::fmt;

// Device -----------------------------------------------------------------------------------------

///
/// Device Representationg
///
/// This struct represents the device. It contains all information about the device and provides
/// functions to read and write data from and to the device.
///
pub struct Device {
    // Flash description
    flash_desc: FlashDesc,

    /// Vector of all entries
    entries: Vec<Entry>,
}

/// Implementation of the Display trait for the Device struct
impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "VID: {:#010X} | PID: {:#010X} | PRD: {:010X}",
            self.get_entry_value(RequestType::DevInfoVID),
            self.get_entry_value(RequestType::DevInfoPID),
            self.get_entry_value(RequestType::DevInfoPRD)
        )
    }
}

impl Device {
    /// Create a new device
    pub fn new() -> Self {
        let mut device = Self {
            flash_desc: FlashDesc::new(0, 0, 0),
            entries: Vec::new(),
        };

        device._add_entry(EntryType::Const, RequestType::DevInfoBootloaderVersion);
        device._add_entry(EntryType::Const, RequestType::DevInfoBootloaderCRC);
        device._add_entry(EntryType::Const, RequestType::DevInfoVID);
        device._add_entry(EntryType::Const, RequestType::DevInfoPID);
        device._add_entry(EntryType::Const, RequestType::DevInfoPRD);
        device._add_entry(EntryType::Const, RequestType::DevInfoUID);

        device._add_entry(EntryType::Const, RequestType::FlashInfoStartAddr);
        device._add_entry(EntryType::Const, RequestType::FlashInfoPageSize);
        device._add_entry(EntryType::Const, RequestType::FlashInfoNumPages);

        device._add_entry(EntryType::Const, RequestType::AppInfoPageIdx);
        device._add_entry(EntryType::RO, RequestType::AppInfoCRCCalc);

        device._add_entry(EntryType::Cmd, RequestType::PageBufferClear);
        device._add_entry(EntryType::RW, RequestType::PageBufferWriteWord);
        device._add_entry(EntryType::RO, RequestType::PageBufferCalcCRC);
        device._add_entry(EntryType::Cmd, RequestType::PageBufferWriteToFlash);

        device._add_entry(EntryType::Cmd, RequestType::FlashWriteErasePage);
        device._add_entry(EntryType::Cmd, RequestType::FlashWriteAppCRC);

        device._add_entry(EntryType::Cmd, RequestType::StartApp);

        device
    }

    /// Initialize the device struct
    ///
    /// This function reads all constant data from the device and stores it in the device struct.
    pub fn init<T: ComInterface>(&mut self, interface: &mut T) -> Result<(), Error> {
        // Read constant data from device
        self._read_const_data(interface)?;

        // Get complete flash description
        let flash_start = self.get_entry_value(RequestType::FlashInfoStartAddr);
        let flash_page_size = self.get_entry_value(RequestType::FlashInfoPageSize);
        let flash_num_pages = self.get_entry_value(RequestType::FlashInfoNumPages);
        let flash_size = flash_page_size * flash_num_pages;
        let flash_app_page_idx = self.get_entry_value(RequestType::AppInfoPageIdx);

        // Calculate bootloader area
        let bootloader_start = flash_start;
        let bootloader_size = flash_app_page_idx * flash_page_size;

        // Calculate application area
        let app_start = flash_start + (flash_app_page_idx * flash_page_size);
        let app_size = flash_size - bootloader_size;

        // Create flash description
        self.flash_desc = FlashDesc::new(flash_start, flash_size, flash_page_size);

        // Add bootloader section
        self.flash_desc
            .add_section("Bootloader", bootloader_start, bootloader_size)
            .map_err(|e| Error::Error(format!("Failed to add bootloader section: {}", e)))?;

        // Add application section
        self.flash_desc
            .add_section("Application", app_start, app_size)
            .map_err(|e| Error::Error(format!("Failed to add application section: {}", e)))?;

        Ok(())
    }

    pub fn flash<T: ComInterface, FWI: FirmwareDataInterface>(
        &mut self,
        interface: &mut T,
        fwi: &FWI,
    ) -> Result<(), Error> {
        // Read necessary data to variables
        let app_section = self.flash_desc.get_section("Application").unwrap();

        let fw_data = fwi.get_firmware_data().unwrap();
        let fw_size = fw_data.len() as u32;
        let fw_num_pages = (fw_size / app_section.get_page_size()) + 1;

        // Print firmware information
        println!(
            "Firmware Data: Size: {:#.2} kB Num Pages: {}",
            (fw_size as f32 / 1024.0),
            fw_num_pages
        );

        // TODO add check if firmware is valid and fits into flash
        // Check page id (min limit)
        // Check firmware size (max limit)

        // Create app firmware representation
        let mut app_fw = AppFirmware::from_section(&app_section);
        app_fw.append_firmware(fw_data)?;

        // Transmit all pages of the firmware to the device
        self._flash_app_pages(interface, &app_fw)?;

        println!("Checking CRC");
        self._check_app_crc(interface, &app_fw)?;

        println!("Flashing App CRC");
        self._flash_app_crc(interface, app_fw.get_crc())?;

        println!("Starting App");
        self.get_entry_mut(RequestType::StartApp)
            .exec(interface, 0)?;

        println!("App successfully flashed & started!");

        Ok(())
    }

    // Getters ------------------------------------------------------------------------------------

    /// Get entry of request type
    ///
    /// This function returns the entry of the given request type. If no entry is found, None is
    /// returned.
    ///
    fn get_entry(&self, request_type: RequestType) -> &Entry {
        for entry in self.entries.iter() {
            if *entry.get_request_type() == request_type {
                return entry;
            }
        }

        panic!("No entry found for request type: {:?}", request_type)
    }

    /// Get mutable entry of request type
    ///
    /// This function returns the mutable entry of the given request type. If no entry is found, None
    /// is returned.
    fn get_entry_mut(&mut self, request_type: RequestType) -> &mut Entry {
        for entry in self.entries.iter_mut() {
            if *entry.get_request_type() == request_type {
                return entry;
            }
        }

        panic!("No entry found for request type: {:?}", request_type);
    }

    /// Get entry value of request type
    ///
    /// This function returns the value of the entry of the given request type. If no entry is found,
    /// the function panics.
    pub fn get_entry_value(&self, request_type: RequestType) -> u32 {
        self.get_entry(request_type).get_value().unwrap().to_word()
    }

    /// Read entry value of request type
    pub fn read_entry_value<T: ComInterface>(
        &mut self,
        interface: &mut T,
        request_type: RequestType,
    ) -> Result<&MsgData, Error> {
        self.get_entry_mut(request_type).read_value(interface)
    }

    pub fn is_app_crc_valid<T: ComInterface>(
        &mut self,
        interface: &mut T,
        app: &AppFirmware,
    ) -> Result<bool, Error> {
        let app_crc = app.get_crc();
        let dev_crc = self
            .read_entry_value(interface, RequestType::AppInfoCRCCalc)?
            .to_word();
        Ok(app_crc == dev_crc)
    }

    // Private Functions --------------------------------------------------------------------------

    fn _add_entry(&mut self, entry_type: EntryType, request_type: RequestType) {
        self.entries.push(Entry::new(entry_type, request_type));
    }

    fn _read_const_data<T: ComInterface>(&mut self, interface: &mut T) -> Result<(), Error> {
        for entry in self.entries.iter_mut() {
            if entry.get_entry_type().is_const() {
                entry.read_value(interface)?;
            }
        }

        return Ok(());
    }

    fn _flash_app_pages<T: ComInterface>(
        &mut self,
        interface: &mut T,
        app: &AppFirmware,
    ) -> Result<(), Error> {
        let mut page_cnt = 1;
        for app_page in app.get_page_lst().iter() {
            let app_section = self.flash_desc.get_section("Application").unwrap();
            let flash_page_id = app_page.get_id() + app_section.get_flash_page_id();

            // Print info
            println!(
                "Flashing {}. page of {}. [Page: {}/{} | Address: {:#08X}]",
                page_cnt,
                app.get_page_lst().len(),
                flash_page_id + 1,
                app.get_flash_num_pages(),
                app_page.get_address()
            );

            // Clear page buffer
            self.get_entry_mut(RequestType::PageBufferClear)
                .exec(interface, 0)?;

            // Write bytes to page buffer
            let fw_page_byte_lst = app_page.get_bytes();

            // One word per message
            for msg_idx in 0..((app.get_flash_page_size() as usize) / 4) {
                let byte_offset = msg_idx * 4;

                // Create data
                let msg_data = MsgData::from_array(&[
                    fw_page_byte_lst[byte_offset],
                    fw_page_byte_lst[byte_offset + 1],
                    fw_page_byte_lst[byte_offset + 2],
                    fw_page_byte_lst[byte_offset + 3],
                ]);

                // Calculate packet id
                let packet_id = (msg_idx % 256) as u8;

                // Write word to page buffer
                self.get_entry_mut(RequestType::PageBufferWriteWord)
                    .write_value(interface, packet_id, &msg_data)?;
            }

            // Read CRC value of page buffer from device
            let page_dev_crc = self
                .read_entry_value(interface, RequestType::PageBufferCalcCRC)?
                .to_word();
            let page_calc_crc = app_page.get_crc();

            if page_dev_crc != page_calc_crc {
                return Err(Error::Error(format!(
                    "Page buffer CRC is invalid! Calc: {:#010X} Dev: {:#010X}!",
                    page_calc_crc, page_dev_crc
                )));
            }

            // Erase flash page
            self.get_entry_mut(RequestType::FlashWriteErasePage)
                .exec(interface, flash_page_id)?;

            // Write page buffer to flash
            self.get_entry_mut(RequestType::PageBufferWriteToFlash)
                .exec(interface, flash_page_id)?;

            page_cnt += 1;
        }

        Ok(())
    }

    fn _erase_unused_pages<T: ComInterface>(
        &mut self,
        interface: &mut T,
        app: &AppFirmware,
    ) -> Result<(), Error> {
        let flash_num_pages = self.get_entry_value(RequestType::FlashInfoNumPages);
        let app_start_page_idx = self.get_entry_value(RequestType::AppInfoPageIdx);

        // Loop through all application pages and check if they are used
        for app_page_id in 0..app.get_page_lst().len() as u32 {
            // Calculate absolute flash page id
            let flash_page_id = app_page_id + app_start_page_idx;

            // Check if page is used
            if app.get_page(app_page_id).is_none() {
                println!(
                    "Erasing unused [Page: {}/{}]",
                    flash_page_id + 1,
                    flash_num_pages
                );

                // Erase flash page
                self.get_entry_mut(RequestType::FlashWriteErasePage)
                    .exec(interface, flash_page_id)?;
            }
        }

        Ok(())
    }

    fn _check_app_crc<T: ComInterface>(
        &mut self,
        interface: &mut T,
        app: &AppFirmware,
    ) -> Result<(), Error> {
        if !self.is_app_crc_valid(interface, app)? {
            // Sometimes the CRC is not valid, because the new app needs less flash
            // then the new one. If the unused flash is not cleared the CRC value will
            // be wrong.
            self._erase_unused_pages(interface, app)?;

            // Check again if CRC is valid
            if !self.is_app_crc_valid(interface, app)? {
                // CRC still invalid throw error
                return Err(Error::Error(format!(
                    "CRC check failed! App-CRC: {:#010X} Device-App-CRC: {:#010X}",
                    app.get_crc(),
                    self.get_entry_value(RequestType::AppInfoCRCCalc)
                )));
            }
        }

        Ok(())
    }

    fn _flash_app_crc<T: ComInterface>(
        &mut self,
        interface: &mut T,
        crc_value: u32,
    ) -> Result<(), Error> {
        self.get_entry_mut(RequestType::FlashWriteAppCRC)
            .exec(interface, crc_value)
    }
}

// Tests ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::francor::franklyboot::com::{
        msg::{Msg, MsgData, ResultType},
        ComSimulator,
    };

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
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoBootloaderCRC)
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoVID)
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoPID)
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoPRD)
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::DevInfoUID)
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::FlashInfoStartAddr)
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::FlashInfoPageSize)
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::FlashInfoNumPages)
            .get_value()
            .is_none());

        assert!(device
            .get_entry(RequestType::AppInfoPageIdx)
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
