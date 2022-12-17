use crate::francor::franklyboot::com::{
    msg::{Msg, RequestType, ResponseType},
    ComError, ComInterface,
};

// Device Entry -----------------------------------------------------------------------------------

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

    pub fn read_from_device(
        &mut self,
        interface: &mut Box<dyn ComInterface>,
    ) -> Result<bool, ComError> {
        // Send request to device
        let request = Msg::new_std_request(self.request_type);
        interface.send(&request)?;

        // Wait for response
        let response = interface.recv()?;
        match response {
            Some(msg) => {
                // Check if response is valid
                let request_valid = msg.get_request() == request.get_request();
                let response_valid = msg.get_response() == ResponseType::RespAck;
                let msg_valid = request_valid && response_valid;

                if msg_valid {
                    self.value = Some(msg.get_data().to_word());
                    return Ok(true);
                } else {
                    self.value = None;
                    return Err(ComError::MsgError(format!(
                        "Error Reading \"{:?}\"\nDevice response is invalid! \
                         TX: Request {:?}\n\tRX: RequestType {:?} ResponseType {:?}",
                        self.name,
                        request.get_request(),
                        msg.get_request(),
                        msg.get_response()
                    )));
                }
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

/*
pub struct Version {
    major: u8,
    minor: u8,
    patch: u8,
}

pub struct DeviceInfo {
    bootloader_version: Version,
    bootloader_crc: u32,
    vendor_id: u32,
    product_id: u32,
    production_date: u32,
    unique_id: u32,
}

pub struct FlashInfo {
    start_address: u32,
    page_size: u32,
    num_pages: u32,
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_entry_new() {
        let entry = DeviceEntry::new(
            "Bootloader Version",
            RequestType::ReqDevInfoBootloaderVersion,
        );

        assert_eq!(entry.name, "Bootloader Version");
        assert_eq!(entry.request_type, RequestType::ReqDevInfoBootloaderVersion);
        assert_eq!(entry.value, None);
    }
}
