use crate::francor::franklyboot::{
    com::{
        self,
        msg::{Msg, MsgData, RequestType},
        ComError, ComInterface,
    },
    firmware::FlashPage,
};

use super::firmware::FirmwareDataInterface;

// Device Entry -----------------------------------------------------------------------------------

#[derive(Debug)]
pub struct DeviceEntry {
    name: String,
    request_type: RequestType,
    value: Option<u32>,
}

impl DeviceEntry {
    pub fn new(name: &str, request_type: RequestType) -> Self {
        DeviceEntry {
            name: name.to_string(),
            request_type: request_type,
            value: None,
        }
    }

    pub fn read_from_device<T: ComInterface>(
        &mut self,
        interface: &mut T,
    ) -> Result<bool, ComError> {
        // Send request to device
        let request = Msg::new_std_request(self.request_type);

        match com::handle_read_data_request(interface, &request)? {
            Some(value) => {
                self.value = Some(value.to_word());
                return Ok(true);
            }
            None => {
                return Ok(false);
            }
        }
    }

    pub fn get_value(&self) -> Option<u32> {
        self.value
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_request_type(&self) -> RequestType {
        self.request_type
    }
}

// Device -----------------------------------------------------------------------------------------

pub struct Device {
    const_data_lst: Vec<DeviceEntry>,
}

impl Device {
    pub fn new() -> Self {
        let mut device = Device {
            const_data_lst: Vec::new(),
        };

        device.add_const_entry("Bootloader Version", RequestType::DevInfoBootloaderVersion);

        device.add_const_entry("Bootloader CRC", RequestType::DevInfoBootloaderCRC);
        device.add_const_entry("Vendor ID", RequestType::DevInfoVID);
        device.add_const_entry("Product ID", RequestType::DevInfoPID);
        device.add_const_entry("Production Date", RequestType::DevInfoPRD);
        device.add_const_entry("Unique ID", RequestType::DevInfoUID);

        device.add_const_entry("Flash Start Address", RequestType::FlashInfoStartAddr);
        device.add_const_entry("Flash Page Size", RequestType::FlashInfoPageSize);
        device.add_const_entry("Flash Number of Pages", RequestType::FlashInfoNumPages);

        device.add_const_entry("App First Page Index", RequestType::AppInfoPageIdx);

        device
    }

    pub fn reset_device<T: ComInterface>(&mut self, interface: &mut T) -> Result<bool, ComError> {
        let reset_request = Msg::new_std_request(RequestType::ResetDevice);
        return com::handle_command_request(interface, &reset_request);
    }

    pub fn start_app<T: ComInterface>(&mut self, interface: &mut T) -> Result<bool, ComError> {
        let start_app_request = Msg::new_std_request(RequestType::StartApp);
        return com::handle_command_request(interface, &start_app_request);
    }

    pub fn flash_firmware<T: ComInterface, FW: FirmwareDataInterface>(
        &mut self,
        interface: &mut T,
        firmware: &FW,
    ) -> Result<(), String> {
        // Read const data
        let firmware_size = firmware.get_firmware_data().unwrap().len() as u32;
        let firmware_pages = firmware_size
            / self
                .get_const_data(RequestType::FlashInfoPageSize)
                .get_value()
                .unwrap();

        let flash_start_address = self
            .get_const_data(RequestType::FlashInfoStartAddr)
            .get_value()
            .unwrap();

        let flash_page_size = self
            .get_const_data(RequestType::FlashInfoPageSize)
            .get_value()
            .unwrap();

        let flash_num_pages = self
            .get_const_data(RequestType::FlashInfoNumPages)
            .get_value()
            .unwrap();

        let app_start_idx = self
            .get_const_data(RequestType::AppInfoPageIdx)
            .get_value()
            .unwrap();

        // Get flash pages
        let flash_pages = FlashPage::from_firmware_data(
            firmware.get_firmware_data().unwrap(),
            flash_start_address,
            flash_page_size,
            flash_num_pages,
        )
        .unwrap();

        // Flash all pages
        let mut page_cnt = 1;
        let mut page_id_lst: Vec<u32> = flash_pages.keys().map(|x| *x).collect();
        for page_id in &page_id_lst {
            let page_id_vld = *page_id >= app_start_idx;

            if !page_id_vld {
                return Err(format!(
                    "Firmware contains invalid address! Minimum page id {} < app start page id {}",
                    page_id, app_start_idx
                ));
            }

            println!(
                "Flashing {}. page of {}. [Page: {}/{} | Address: {:#08X}]",
                page_cnt,
                page_id_lst.len(),
                page_id,
                flash_num_pages,
                flash_pages[&page_id].get_address()
            );

            // Clear page buffer
            match self.clear_page_buffer(interface) {
                Ok(result) => {
                    if !result {
                        return Err(format!("Failed to clear page buffer!"));
                    }
                }
                Err(e) => {
                    return Err(format!("Failed to clear page buffer! Error: {:#?}", e));
                }
            }

            // Write bytes to page buffer
            let byte_lst = flash_pages[page_id].get_bytes();
            for msg_idx in 0..((flash_page_size as usize) / 4) {
                let byte_offset = msg_idx * 4;

                let data = MsgData::from_array(&[
                    byte_lst[byte_offset + 0],
                    byte_lst[byte_offset + 1],
                    byte_lst[byte_offset + 2],
                    byte_lst[byte_offset + 3],
                ]);

                // Write word to buffer -> calculate packet ID
                let packet_id = (msg_idx % 256) as u8;

                match com::hande_write_request(
                    interface,
                    RequestType::PageBufferWriteWord,
                    packet_id,
                    &data,
                ) {
                    Ok(result) => {
                        if !result {
                            return Err(format!(
                                "Failed to transmit word to flash buffer! Message timeout!"
                            ));
                        }
                    }
                    Err(e) => {
                        return Err(format!(
                            "Failed to transmit word to flash buffer!\n{:#?}",
                            e
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn read_const_data<T: ComInterface>(&mut self, interface: &mut T) -> Result<(), ComError> {
        for entry in self.const_data_lst.iter_mut() {
            entry.read_from_device(interface)?;
        }

        Ok(())
    }

    pub fn get_const_data(&self, request_type: RequestType) -> &DeviceEntry {
        for entry in self.const_data_lst.iter() {
            if entry.get_request_type() == request_type {
                return &entry;
            }
        }

        panic!("Invalid request type specified for get_const_data!");
    }

    fn add_const_entry(&mut self, name: &str, request_type: RequestType) {
        self.const_data_lst
            .push(DeviceEntry::new(name, request_type));
    }

    pub fn clear_page_buffer<T: ComInterface>(
        &mut self,
        interface: &mut T,
    ) -> Result<bool, ComError> {
        let clear_page_buffer_request = Msg::new_std_request(RequestType::PageBufferClear);
        return com::handle_command_request(interface, &clear_page_buffer_request);
    }
}

// Tests ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::francor::franklyboot::com::{
        msg::{MsgData, ResponseType},
        ComSimulator,
    };

    #[test]
    fn device_entry_new() {
        let entry = DeviceEntry::new("Bootloader Version", RequestType::DevInfoBootloaderVersion);

        assert_eq!(entry.name, "Bootloader Version");
        assert_eq!(entry.request_type, RequestType::DevInfoBootloaderVersion);
        assert_eq!(entry.value, None);
    }

    #[test]
    fn device_entry_read() {
        let mut entry =
            DeviceEntry::new("Bootloader Version", RequestType::DevInfoBootloaderVersion);

        let mut com = ComSimulator::new();
        com.add_response(Msg::new(
            RequestType::DevInfoBootloaderVersion,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x01020304),
        ));

        let result = entry.read_from_device(&mut com);
        assert_eq!(result, Ok(true));
        assert_eq!(entry.value, Some(0x01020304));
    }

    #[test]
    fn device_entry_read_send_error() {
        let mut entry =
            DeviceEntry::new("Bootloader Version", RequestType::DevInfoBootloaderVersion);

        let mut com = ComSimulator::new();
        com.add_response(Msg::new(
            RequestType::DevInfoBootloaderVersion,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x01020304),
        ));
        com.set_send_error(ComError::Error("Send error".to_string()));

        let result = entry.read_from_device(&mut com);
        assert_eq!(result, Err(ComError::Error("Send error".to_string())));
        assert_eq!(entry.value, None);
    }

    #[test]
    fn device_entry_read_recv_error() {
        let mut entry =
            DeviceEntry::new("Bootloader Version", RequestType::DevInfoBootloaderVersion);

        let mut com = ComSimulator::new();
        com.add_response(Msg::new(
            RequestType::DevInfoBootloaderVersion,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x01020304),
        ));
        com.set_recv_error(ComError::Error("Recv error".to_string()));

        let result = entry.read_from_device(&mut com);
        assert_eq!(result, Err(ComError::Error("Recv error".to_string())));
        assert_eq!(entry.value, None);
    }

    #[test]
    fn device_entry_read_recv_timeout() {
        let mut entry =
            DeviceEntry::new("Bootloader Version", RequestType::DevInfoBootloaderVersion);

        let mut com = ComSimulator::new();
        com.add_response(Msg::new(
            RequestType::DevInfoBootloaderVersion,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x01020304),
        ));
        com.set_recv_timeout_error();

        let result = entry.read_from_device(&mut com);
        assert_eq!(result, Ok(false));
        assert_eq!(entry.value, None);
    }

    #[test]
    fn device_read_const_data() {
        // Will not work, because hash map will not sort by value

        let mut device = Device::new();

        let mut com = ComSimulator::new();
        com.add_response(Msg::new(
            RequestType::DevInfoBootloaderVersion,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x01020304),
        ));

        com.add_response(Msg::new(
            RequestType::DevInfoBootloaderCRC,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x05060708),
        ));

        com.add_response(Msg::new(
            RequestType::DevInfoVID,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0xDEADBEFF),
        ));

        com.add_response(Msg::new(
            RequestType::DevInfoPID,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x1),
        ));

        com.add_response(Msg::new(
            RequestType::DevInfoPRD,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x2),
        ));

        com.add_response(Msg::new(
            RequestType::DevInfoUID,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x11223344),
        ));

        com.add_response(Msg::new(
            RequestType::FlashInfoStartAddr,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x08000000),
        ));

        com.add_response(Msg::new(
            RequestType::FlashInfoPageSize,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(0x0800),
        ));

        com.add_response(Msg::new(
            RequestType::FlashInfoNumPages,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(16),
        ));

        com.add_response(Msg::new(
            RequestType::AppInfoPageIdx,
            ResponseType::RespAck,
            0,
            &MsgData::from_word(8),
        ));

        // Read data from device
        assert!(device.read_const_data(&mut com).is_ok());

        // Check const data
        assert_eq!(
            device
                .get_const_data(RequestType::DevInfoBootloaderVersion)
                .get_value(),
            Some(0x01020304)
        );

        assert_eq!(
            device
                .get_const_data(RequestType::DevInfoBootloaderCRC)
                .get_value(),
            Some(0x05060708)
        );

        assert_eq!(
            device.get_const_data(RequestType::DevInfoVID).get_value(),
            Some(0xDEADBEFF)
        );

        assert_eq!(
            device.get_const_data(RequestType::DevInfoPID).get_value(),
            Some(0x1)
        );

        assert_eq!(
            device.get_const_data(RequestType::DevInfoPRD).get_value(),
            Some(0x2)
        );

        assert_eq!(
            device.get_const_data(RequestType::DevInfoUID).get_value(),
            Some(0x11223344)
        );

        assert_eq!(
            device
                .get_const_data(RequestType::FlashInfoStartAddr)
                .get_value(),
            Some(0x08000000)
        );

        assert_eq!(
            device
                .get_const_data(RequestType::FlashInfoPageSize)
                .get_value(),
            Some(0x0800)
        );

        assert_eq!(
            device
                .get_const_data(RequestType::FlashInfoNumPages)
                .get_value(),
            Some(16)
        );

        assert_eq!(
            device
                .get_const_data(RequestType::AppInfoPageIdx)
                .get_value(),
            Some(8)
        );
    }
}
