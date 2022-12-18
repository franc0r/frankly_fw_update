// Request Type -----------------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq)]
pub enum RequestType {
    Ping,        //< Ping device | Response is bootloader version
    ResetDevice, //< Resets the device (hardware reset)
    StartApp,    //< Start application and exit bootloader

    /* Device information */
    DevInfoBootloaderVersion, //< Reads the bootloader version
    DevInfoBootloaderCRC,     //< Calculates the CRC of the bootloader flash area
    DevInfoVID,               //< Reads the vendor id
    DevInfoPID,               //< Reads the product id
    DevInfoPRD,               //< Reads the production date
    DevInfoUID,               //< Reads the device unique ID

    /* Flash information */
    FlashInfoStartAddr, //< Get the start address of the flash area
    FlashInfoPageSize,  //< Get the size in bytes of a page
    FlashInfoNumPages,  //< Get the number of pages (including bootloader area)

    /* App Information */
    AppInfoPageIdx, //< Get the page idx of app area in flash
    AppInfoCRCCalc, //< Get the calculate CRC over app flash area
    AppInfoCRCStrd, //< Get the stored CRC value used for safe startup

    /* Flash Read commands */
    FlashReadWord, //< Reads a word from the flash

    /* Page Buffer Commands */
    PageBufferClear,        //< Clears the page buffer (RAM)
    PageBufferReadWord,     //< Reads a word to the page buffer (RAM)
    PageBufferWriteWord,    //< Writes a word to the page buffer (RAM)
    PageBufferCalcCRC,      //< Calculates the CRC over the page buffer
    PageBufferWriteToFlash, //< Write the page buffer to the desired flash page

    /* Flash Write Commands*/
    FlashWriteErasePage, //< Erases an flash page
    FlashWriteAppCRC,    //< Writes the CRC of the app to the flash
}

impl RequestType {
    pub fn from_u16(value: u16) -> RequestType {
        match value {
            0x0001 => RequestType::Ping,
            0x0011 => RequestType::ResetDevice,
            0x0012 => RequestType::StartApp,
            0x0101 => RequestType::DevInfoBootloaderVersion,
            0x0102 => RequestType::DevInfoBootloaderCRC,
            0x0103 => RequestType::DevInfoVID,
            0x0104 => RequestType::DevInfoPID,
            0x0105 => RequestType::DevInfoPRD,
            0x0106 => RequestType::DevInfoUID,
            0x0201 => RequestType::FlashInfoStartAddr,
            0x0202 => RequestType::FlashInfoPageSize,
            0x0203 => RequestType::FlashInfoNumPages,
            0x0301 => RequestType::AppInfoPageIdx,
            0x0302 => RequestType::AppInfoCRCCalc,
            0x0303 => RequestType::AppInfoCRCStrd,
            0x0401 => RequestType::FlashReadWord,
            0x1001 => RequestType::PageBufferClear,
            0x1002 => RequestType::PageBufferReadWord,
            0x1003 => RequestType::PageBufferWriteWord,
            0x1004 => RequestType::PageBufferCalcCRC,
            0x1005 => RequestType::PageBufferWriteToFlash,
            0x1101 => RequestType::FlashWriteErasePage,
            0x1102 => RequestType::FlashWriteAppCRC,
            _ => panic!("Unknown request type: {}", value),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            RequestType::Ping => 0x0001,
            RequestType::ResetDevice => 0x0011,
            RequestType::StartApp => 0x0012,
            RequestType::DevInfoBootloaderVersion => 0x0101,
            RequestType::DevInfoBootloaderCRC => 0x0102,
            RequestType::DevInfoVID => 0x0103,
            RequestType::DevInfoPID => 0x0104,
            RequestType::DevInfoPRD => 0x0105,
            RequestType::DevInfoUID => 0x0106,
            RequestType::FlashInfoStartAddr => 0x0201,
            RequestType::FlashInfoPageSize => 0x0202,
            RequestType::FlashInfoNumPages => 0x0203,
            RequestType::AppInfoPageIdx => 0x0301,
            RequestType::AppInfoCRCCalc => 0x0302,
            RequestType::AppInfoCRCStrd => 0x0303,
            RequestType::FlashReadWord => 0x0401,
            RequestType::PageBufferClear => 0x1001,
            RequestType::PageBufferReadWord => 0x1002,
            RequestType::PageBufferWriteWord => 0x1003,
            RequestType::PageBufferCalcCRC => 0x1004,
            RequestType::PageBufferWriteToFlash => 0x1005,
            RequestType::FlashWriteErasePage => 0x1101,
            RequestType::FlashWriteAppCRC => 0x1102,
        }
    }
}

// Response types ---------------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ResponseType {
    RespNone, // Unused / ignored
    RespAck,  // Acknowledge

    /* Errors */
    RespErr,             // General error
    RespUnknownReq,      // Unknow request type
    RespErrNotSupported, // Error, command known but not supported
    RespErrCRCInvld,     // Error, CRC check failed
    RespAckPageFull,     // Acknowledge and info that page buffer is full
    RespErrPageFull,     // Error, word not writable page buffer is full
    RespErrInvldArg,     // Error, invalid argument (out of range, ...)
}

impl ResponseType {
    pub fn to_u8(&self) -> u8 {
        match self {
            ResponseType::RespNone => 0x00,
            ResponseType::RespAck => 0x01,
            ResponseType::RespErr => 0xFE,
            ResponseType::RespUnknownReq => 0xFD,
            ResponseType::RespErrNotSupported => 0xFC,
            ResponseType::RespErrCRCInvld => 0xFB,
            ResponseType::RespAckPageFull => 0xFA,
            ResponseType::RespErrPageFull => 0xF9,
            ResponseType::RespErrInvldArg => 0xF8,
        }
    }

    pub fn from_u8(value: u8) -> ResponseType {
        match value {
            0x00 => ResponseType::RespNone,
            0x01 => ResponseType::RespAck,
            0xFE => ResponseType::RespErr,
            0xFD => ResponseType::RespUnknownReq,
            0xFC => ResponseType::RespErrNotSupported,
            0xFB => ResponseType::RespErrCRCInvld,
            0xFA => ResponseType::RespAckPageFull,
            0xF9 => ResponseType::RespErrPageFull,
            0xF8 => ResponseType::RespErrInvldArg,
            _ => panic!("Unknown response type: {}", value),
        }
    }
}

// Message Data -----------------------------------------------------------------------------------
pub type MsgDataRaw = [u8; 4];

#[derive(Debug, Clone)]
pub struct MsgData {
    data: MsgDataRaw,
}

impl MsgData {
    pub fn new() -> MsgData {
        MsgData { data: [0; 4] }
    }

    pub fn from_array(data: &MsgDataRaw) -> MsgData {
        MsgData { data: *data }
    }

    pub fn from_word(value: u32) -> MsgData {
        MsgData {
            data: [
                (value & 0x000000FF) as u8,
                ((value & 0x0000FF00) >> 8) as u8,
                ((value & 0x00FF0000) >> 16) as u8,
                ((value & 0xFF000000) >> 24) as u8,
            ],
        }
    }

    pub fn to_word(&self) -> u32 {
        (self.data[0] as u32)
            | ((self.data[1] as u32) << 8)
            | ((self.data[2] as u32) << 16)
            | ((self.data[3] as u32) << 24)
    }

    pub fn get_byte(&self, idx: usize) -> u8 {
        self.data[idx]
    }

    pub fn get_array(&self) -> MsgDataRaw {
        self.data
    }
}

// Message ----------------------------------------------------------------------------------------

pub type MsgRaw = [u8; 8];

#[derive(Debug)]
pub struct Msg {
    pub request: RequestType,
    pub response: ResponseType,
    pub packet_id: u8,
    pub data: MsgData,
}

impl Msg {
    pub fn new(request: RequestType, response: ResponseType, packet_id: u8, data: &MsgData) -> Msg {
        Msg {
            request: request,
            response: response,
            packet_id: packet_id,
            data: data.clone(),
        }
    }

    pub fn new_std_request(request: RequestType) -> Msg {
        Msg {
            request: request,
            response: ResponseType::RespNone,
            packet_id: 0,
            data: MsgData::new(),
        }
    }

    pub fn from_raw_data_array(data: &MsgRaw) -> Msg {
        let request = RequestType::from_u16((data[0] as u16) | ((data[1] as u16) << 8));
        let response = ResponseType::from_u8(data[2]);
        let packet_id = data[3];
        let data = MsgData::from_array(&[data[4], data[5], data[6], data[7]]);

        Msg {
            request: request,
            response: response,
            packet_id: packet_id,
            data: data,
        }
    }

    pub fn to_raw_data_array(&self) -> MsgRaw {
        let mut data: MsgRaw = [0; 8];
        data[0] = (self.request.to_u16() & 0x00FF) as u8;
        data[1] = ((self.request.to_u16() & 0xFF00) >> 8) as u8;
        data[2] = self.response.to_u8();
        data[3] = self.packet_id;
        data[4] = self.data.get_byte(0);
        data[5] = self.data.get_byte(1);
        data[6] = self.data.get_byte(2);
        data[7] = self.data.get_byte(3);

        data
    }

    pub fn get_request(&self) -> RequestType {
        self.request
    }

    pub fn get_response(&self) -> ResponseType {
        self.response
    }

    pub fn get_packet_id(&self) -> u8 {
        self.packet_id
    }

    pub fn get_data(&self) -> &MsgData {
        &self.data
    }
}

// Tests ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_type_convert_to_u16() {
        assert_eq!(RequestType::Ping.to_u16(), 0x0001);
        assert_eq!(RequestType::ResetDevice.to_u16(), 0x0011);
        assert_eq!(RequestType::StartApp.to_u16(), 0x0012);
        assert_eq!(RequestType::DevInfoBootloaderVersion.to_u16(), 0x0101);
        assert_eq!(RequestType::DevInfoBootloaderCRC.to_u16(), 0x0102);
        assert_eq!(RequestType::DevInfoVID.to_u16(), 0x0103);
        assert_eq!(RequestType::DevInfoPID.to_u16(), 0x0104);
        assert_eq!(RequestType::DevInfoPRD.to_u16(), 0x0105);
        assert_eq!(RequestType::DevInfoUID.to_u16(), 0x0106);
        assert_eq!(RequestType::FlashInfoStartAddr.to_u16(), 0x0201);
        assert_eq!(RequestType::FlashInfoPageSize.to_u16(), 0x0202);
        assert_eq!(RequestType::FlashInfoNumPages.to_u16(), 0x0203);
        assert_eq!(RequestType::AppInfoPageIdx.to_u16(), 0x0301);
        assert_eq!(RequestType::AppInfoCRCCalc.to_u16(), 0x0302);
        assert_eq!(RequestType::AppInfoCRCStrd.to_u16(), 0x0303);
        assert_eq!(RequestType::FlashReadWord.to_u16(), 0x0401);
        assert_eq!(RequestType::PageBufferClear.to_u16(), 0x1001);
        assert_eq!(RequestType::PageBufferReadWord.to_u16(), 0x1002);
        assert_eq!(RequestType::PageBufferWriteWord.to_u16(), 0x1003);
        assert_eq!(RequestType::PageBufferCalcCRC.to_u16(), 0x1004);
        assert_eq!(RequestType::PageBufferWriteToFlash.to_u16(), 0x1005);
        assert_eq!(RequestType::FlashWriteErasePage.to_u16(), 0x1101);
        assert_eq!(RequestType::FlashWriteAppCRC.to_u16(), 0x1102);
    }

    #[test]
    fn request_type_convert_from_u16() {
        assert_eq!(RequestType::from_u16(0x0001), RequestType::Ping);
        assert_eq!(RequestType::from_u16(0x0011), RequestType::ResetDevice);
        assert_eq!(RequestType::from_u16(0x0012), RequestType::StartApp);
        assert_eq!(
            RequestType::from_u16(0x0101),
            RequestType::DevInfoBootloaderVersion
        );
        assert_eq!(
            RequestType::from_u16(0x0102),
            RequestType::DevInfoBootloaderCRC
        );
        assert_eq!(RequestType::from_u16(0x0103), RequestType::DevInfoVID);
        assert_eq!(RequestType::from_u16(0x0104), RequestType::DevInfoPID);
        assert_eq!(RequestType::from_u16(0x0105), RequestType::DevInfoPRD);
        assert_eq!(RequestType::from_u16(0x0106), RequestType::DevInfoUID);
        assert_eq!(
            RequestType::from_u16(0x0201),
            RequestType::FlashInfoStartAddr
        );
        assert_eq!(
            RequestType::from_u16(0x0202),
            RequestType::FlashInfoPageSize
        );
        assert_eq!(
            RequestType::from_u16(0x0203),
            RequestType::FlashInfoNumPages
        );
        assert_eq!(RequestType::from_u16(0x0301), RequestType::AppInfoPageIdx);
        assert_eq!(RequestType::from_u16(0x0302), RequestType::AppInfoCRCCalc);
        assert_eq!(RequestType::from_u16(0x0303), RequestType::AppInfoCRCStrd);
        assert_eq!(RequestType::from_u16(0x0401), RequestType::FlashReadWord);
        assert_eq!(RequestType::from_u16(0x1001), RequestType::PageBufferClear);
        assert_eq!(
            RequestType::from_u16(0x1002),
            RequestType::PageBufferReadWord
        );
        assert_eq!(
            RequestType::from_u16(0x1003),
            RequestType::PageBufferWriteWord
        );
        assert_eq!(
            RequestType::from_u16(0x1004),
            RequestType::PageBufferCalcCRC
        );
        assert_eq!(
            RequestType::from_u16(0x1005),
            RequestType::PageBufferWriteToFlash
        );
        assert_eq!(
            RequestType::from_u16(0x1101),
            RequestType::FlashWriteErasePage
        );
        assert_eq!(RequestType::from_u16(0x1102), RequestType::FlashWriteAppCRC);
    }

    #[test]
    fn response_convert_to_u8() {
        assert_eq!(ResponseType::RespNone.to_u8(), 0x00);
        assert_eq!(ResponseType::RespAck.to_u8(), 0x01);
        assert_eq!(ResponseType::RespErr.to_u8(), 0xFE);
        assert_eq!(ResponseType::RespUnknownReq.to_u8(), 0xFD);
        assert_eq!(ResponseType::RespErrNotSupported.to_u8(), 0xFC);
        assert_eq!(ResponseType::RespErrCRCInvld.to_u8(), 0xFB);
        assert_eq!(ResponseType::RespAckPageFull.to_u8(), 0xFA);
        assert_eq!(ResponseType::RespErrPageFull.to_u8(), 0xF9);
        assert_eq!(ResponseType::RespErrInvldArg.to_u8(), 0xF8);
    }

    #[test]
    fn response_convert_from_u8() {
        assert_eq!(ResponseType::from_u8(0x00), ResponseType::RespNone);
        assert_eq!(ResponseType::from_u8(0x01), ResponseType::RespAck);
        assert_eq!(ResponseType::from_u8(0xFE), ResponseType::RespErr);
        assert_eq!(ResponseType::from_u8(0xFD), ResponseType::RespUnknownReq);
        assert_eq!(
            ResponseType::from_u8(0xFC),
            ResponseType::RespErrNotSupported
        );
        assert_eq!(ResponseType::from_u8(0xFB), ResponseType::RespErrCRCInvld);
        assert_eq!(ResponseType::from_u8(0xFA), ResponseType::RespAckPageFull);
        assert_eq!(ResponseType::from_u8(0xF9), ResponseType::RespErrPageFull);
        assert_eq!(ResponseType::from_u8(0xF8), ResponseType::RespErrInvldArg);
    }

    #[test]
    fn msg_data_new() {
        assert_eq!(MsgData::new().get_array(), [0; 4]);
    }

    #[test]
    fn msg_data_from_array() {
        assert_eq!(MsgData::from_array(&[1, 2, 3, 4]).get_array(), [1, 2, 3, 4]);
    }

    #[test]
    fn msg_data_from_word() {
        assert_eq!(MsgData::from_word(0x01020304).get_array(), [4, 3, 2, 1]);
    }

    #[test]
    fn msg_data_to_word() {
        assert_eq!(MsgData::from_array(&[4, 3, 2, 1]).to_word(), 0x01020304);
    }

    #[test]
    fn msg_data_get_byte() {
        assert_eq!(MsgData::from_array(&[1, 2, 3, 4]).get_byte(0), 1);
        assert_eq!(MsgData::from_array(&[1, 2, 3, 4]).get_byte(1), 2);
        assert_eq!(MsgData::from_array(&[1, 2, 3, 4]).get_byte(2), 3);
        assert_eq!(MsgData::from_array(&[1, 2, 3, 4]).get_byte(3), 4);
    }

    #[test]
    fn msg_new() {
        let msg = Msg::new(
            RequestType::Ping,
            ResponseType::RespAck,
            5,
            &MsgData::from_array(&[0x01, 0x02, 0x03, 0x04]),
        );
        assert_eq!(msg.request, RequestType::Ping);
        assert_eq!(msg.response, ResponseType::RespAck);
        assert_eq!(msg.packet_id, 5);
        assert_eq!(msg.data.get_array(), [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn msg_from_raw_data_array() {
        let msg = Msg::from_raw_data_array(&[0x03, 0x01, 0x01, 0x05, 0x01, 0x02, 0x03, 0x04]);
        assert_eq!(msg.request, RequestType::DevInfoVID);
        assert_eq!(msg.response, ResponseType::RespAck);
        assert_eq!(msg.packet_id, 5);
        assert_eq!(msg.data.get_array(), [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn msg_to_raw_data_array() {
        let msg = Msg::new(
            RequestType::DevInfoVID,
            ResponseType::RespAck,
            5,
            &MsgData::from_array(&[0x01, 0x02, 0x03, 0x04]),
        );
        assert_eq!(
            msg.to_raw_data_array(),
            [0x03, 0x01, 0x01, 0x05, 0x01, 0x02, 0x03, 0x04]
        );
    }
}
