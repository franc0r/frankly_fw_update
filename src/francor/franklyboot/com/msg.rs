use crate::francor::franklyboot::Error;
use std::fmt;

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
    FlashWriteErasePage, //< Erases a flash page
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

// Result types -----------------------------------------------------------------------------------

/// This enumeration describes the possible result types of the bootloader.
///
/// Every request generates a response from the device which contains the result type.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ResultType {
    None, // No result / not specified
    Ok,   // Message was processed successfully / result ok

    /* Errors */
    Error,           // General error
    ErrUnknownReq,   // Unknow request type
    ErrNotSupported, // Error, command known but not supported
    ErrCRCInvld,     // Error, CRC check failed
    ErrPageFull,     // Error, word not writable page buffer is full
    ErrInvldArg,     // Error, invalid argument (out of range, ...)
}

impl ResultType {
    /// Converts the result type to a u8 value
    pub fn to_u8(&self) -> u8 {
        match self {
            ResultType::None => 0x00,
            ResultType::Ok => 0x01,
            ResultType::Error => 0xFE,
            ResultType::ErrUnknownReq => 0xFD,
            ResultType::ErrNotSupported => 0xFC,
            ResultType::ErrCRCInvld => 0xFB,
            ResultType::ErrPageFull => 0xFA,
            ResultType::ErrInvldArg => 0xF9,
        }
    }

    /// Converts a u8 value to a result type
    pub fn from_u8(value: u8) -> ResultType {
        match value {
            0x00 => ResultType::None,
            0x01 => ResultType::Ok,
            0xFE => ResultType::Error,
            0xFD => ResultType::ErrUnknownReq,
            0xFC => ResultType::ErrNotSupported,
            0xFB => ResultType::ErrCRCInvld,
            0xFA => ResultType::ErrPageFull,
            0xF9 => ResultType::ErrInvldArg,
            _ => panic!("Unknown result type: {}", value),
        }
    }

    /// Returns true if the result type is a success
    pub fn is_ok(&self) -> bool {
        match self {
            ResultType::None => true,
            ResultType::Ok => true,
            _ => false,
        }
    }

    /// Returns true if the result type is an error
    pub fn is_error(&self) -> bool {
        match self {
            ResultType::Error => true,
            ResultType::ErrUnknownReq => true,
            ResultType::ErrNotSupported => true,
            ResultType::ErrCRCInvld => true,
            ResultType::ErrPageFull => true,
            ResultType::ErrInvldArg => true,
            _ => false,
        }
    }
}

/// Implementation of the Display trait for the EntryType enumeration
impl fmt::Display for ResultType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResultType::None => write!(f, "None: No Result specified!"),
            ResultType::Ok => write!(f, "Ok: Message was processed successfully!"),
            ResultType::Error => write!(f, "Error: General error!"),
            ResultType::ErrUnknownReq => write!(f, "Error: Unknown request type!"),
            ResultType::ErrNotSupported => write!(f, "Error: Command known but not supported!"),
            ResultType::ErrCRCInvld => write!(f, "Error: CRC check failed!"),
            ResultType::ErrPageFull => write!(f, "Error: Word not writable page buffer is full!"),
            ResultType::ErrInvldArg => write!(f, "Error: Invalid argument (out of range, ...)!"),
        }
    }
}

// Message Data -----------------------------------------------------------------------------------

/// Raw data type of message payload data
pub type MsgDataRaw = [u8; 4];

/// This structure represents the payload data of a message
///
/// The payload data is a 32-bit word which can be accessed as a byte array or as a single word.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsgData {
    data: MsgDataRaw,
}

impl MsgData {
    /// Creates a new message data object
    pub fn new() -> MsgData {
        MsgData { data: [0; 4] }
    }

    /// Creates a new message data object from a byte array
    pub fn from_array(data: &MsgDataRaw) -> MsgData {
        MsgData { data: *data }
    }

    /// Creates a new message data object from a word
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

    /// Returns the message data as a 32-bit word
    pub fn to_word(&self) -> u32 {
        (self.data[0] as u32)
            | ((self.data[1] as u32) << 8)
            | ((self.data[2] as u32) << 16)
            | ((self.data[3] as u32) << 24)
    }

    /// Returns the data value at the specified index
    pub fn get_byte(&self, idx: usize) -> u8 {
        self.data[idx]
    }

    /// Get the message data as a byte array
    pub fn get_array(&self) -> &MsgDataRaw {
        &self.data
    }
}

// Message ----------------------------------------------------------------------------------------

/// Raw data type of a complete message
///
/// Message is represented by a eight byte data array
pub type MsgRaw = [u8; 8];

/// This structure represents a message
///
/// A message consists of a request type, a result type, a packet id and a payload data word.
#[derive(Debug)]
pub struct Msg {
    pub request: RequestType,
    pub result: ResultType,
    pub packet_id: u8,
    pub data: MsgData,
}

impl Msg {
    /// Creates a new message object
    pub fn new(request: RequestType, result: ResultType, packet_id: u8, data: &MsgData) -> Msg {
        Msg {
            request: request,
            result: result,
            packet_id: packet_id,
            data: data.clone(),
        }
    }

    /// Creates a new message object with a standard request type
    ///
    /// The result type is set to `ResultType::None` and the packet id is set to 0.
    /// The payload data is set to 0.
    pub fn new_std_request(request: RequestType) -> Msg {
        Msg {
            request: request,
            result: ResultType::None,
            packet_id: 0,
            data: MsgData::new(),
        }
    }

    /// Create a new message object from a raw data array (raw message)
    pub fn from_raw_data_array(data: &MsgRaw) -> Msg {
        let request = RequestType::from_u16((data[0] as u16) | ((data[1] as u16) << 8));
        let result = ResultType::from_u8(data[2]);
        let packet_id = data[3];
        let data = MsgData::from_array(&[data[4], data[5], data[6], data[7]]);

        Msg {
            request: request,
            result: result,
            packet_id: packet_id,
            data: data,
        }
    }

    /// Converts a message object to a raw data array (raw message)
    pub fn to_raw_data_array(&self) -> MsgRaw {
        let mut data: MsgRaw = [0; 8];
        data[0] = (self.request.to_u16() & 0x00FF) as u8;
        data[1] = ((self.request.to_u16() & 0xFF00) >> 8) as u8;
        data[2] = self.result.to_u8();
        data[3] = self.packet_id;
        data[4] = self.data.get_byte(0);
        data[5] = self.data.get_byte(1);
        data[6] = self.data.get_byte(2);
        data[7] = self.data.get_byte(3);

        data
    }

    /// Check if response message is ok
    ///
    /// This function checks if the response message is ok.
    /// The message is valid if:
    /// - The request type of the response message is equal to the request type of the request message
    /// - The result type of the response message is ok
    /// - The packet id of the response message is equal to the packet id of the request message
    ///
    /// Data of the request message is not checked!
    pub fn is_response_ok(&self, response: &Msg) -> Result<(), Error> {
        let request_valid = self.request == response.request;
        let result_ok = response.result.is_ok();
        let packet_id_valid = self.packet_id == response.packet_id;
        let msg_valid = request_valid && result_ok && packet_id_valid;

        if msg_valid {
            Ok(())
        } else {
            // Generate error description for better debugging
            let error_info = format!(
                "TX: Request: {:?}, Packet-ID: {}, Data: {:?}\n\
                 RX: Request: {:?}, Packet-ID: {}, Result: {:?}, Data: {:?}",
                self.request,
                self.packet_id,
                self.data,
                response.request,
                response.packet_id,
                response.result,
                response.data
            );

            if result_ok == false {
                Err(Error::ResultError(format!(
                    "Request \"{:?}\" returned error result {}\n{}",
                    self.request, response.result, error_info
                )))
            } else if packet_id_valid == false {
                Err(Error::MsgCorruption(format!(
                    "Message response corrupted packet id invalid!\n{}",
                    error_info
                )))
            } else if request_valid == false {
                Err(Error::MsgCorruption(format!(
                    "Request type mismatch! Expected: \"{:?}\", Received: \"{:?}\"\n{}",
                    self.request, response.request, error_info
                )))
            } else {
                Err(Error::Error(format!(
                    "Unknown message error!\n{}",
                    error_info
                )))
            }
        }
    }

    pub fn is_response_data_ok(&self, response: &Msg) -> Result<(), Error> {
        let data_valid = self.data == response.data;

        if data_valid {
            Ok(())
        } else {
            // Generate error description for better debugging
            let error_info = format!(
                "TX: Request: {:?}, Packet-ID: {}, Data: {:?}\n\
                 RX: Request: {:?}, Packet-ID: {}, Result: {:?}, Data: {:?}",
                self.request,
                self.packet_id,
                self.data,
                response.request,
                response.packet_id,
                response.result,
                response.data
            );

            Err(Error::MsgCorruption(format!("Message response data is invalid! Message seems corrupted or critical error in device!\n{}", error_info)))
        }
    }

    /// Returns the request type of the message
    pub fn get_request(&self) -> RequestType {
        self.request
    }

    /// Returns the result type of the message
    pub fn get_result(&self) -> ResultType {
        self.result
    }

    /// Returns the packet id of the message
    pub fn get_packet_id(&self) -> u8 {
        self.packet_id
    }

    /// Returns the payload data of the message
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
    fn result_convert_to_u8() {
        assert_eq!(ResultType::None.to_u8(), 0x00);
        assert_eq!(ResultType::Ok.to_u8(), 0x01);
        assert_eq!(ResultType::Error.to_u8(), 0xFE);
        assert_eq!(ResultType::ErrUnknownReq.to_u8(), 0xFD);
        assert_eq!(ResultType::ErrNotSupported.to_u8(), 0xFC);
        assert_eq!(ResultType::ErrCRCInvld.to_u8(), 0xFB);
        assert_eq!(ResultType::ErrPageFull.to_u8(), 0xFA);
        assert_eq!(ResultType::ErrInvldArg.to_u8(), 0xF9);
    }

    #[test]
    fn result_convert_from_u8() {
        assert_eq!(ResultType::from_u8(0x00), ResultType::None);
        assert_eq!(ResultType::from_u8(0x01), ResultType::Ok);
        assert_eq!(ResultType::from_u8(0xFE), ResultType::Error);
        assert_eq!(ResultType::from_u8(0xFD), ResultType::ErrUnknownReq);
        assert_eq!(ResultType::from_u8(0xFC), ResultType::ErrNotSupported);
        assert_eq!(ResultType::from_u8(0xFB), ResultType::ErrCRCInvld);
        assert_eq!(ResultType::from_u8(0xFA), ResultType::ErrPageFull);
        assert_eq!(ResultType::from_u8(0xF9), ResultType::ErrInvldArg);
    }

    #[test]
    fn result_is_ok() {
        assert_eq!(ResultType::None.is_ok(), true);
        assert_eq!(ResultType::Ok.is_ok(), true);
        assert_eq!(ResultType::Error.is_ok(), false);
        assert_eq!(ResultType::ErrUnknownReq.is_ok(), false);
        assert_eq!(ResultType::ErrNotSupported.is_ok(), false);
        assert_eq!(ResultType::ErrCRCInvld.is_ok(), false);
        assert_eq!(ResultType::ErrPageFull.is_ok(), false);
        assert_eq!(ResultType::ErrInvldArg.is_ok(), false);
    }

    #[test]
    fn result_is_error() {
        assert_eq!(ResultType::None.is_error(), false);
        assert_eq!(ResultType::Ok.is_error(), false);
        assert_eq!(ResultType::Error.is_error(), true);
        assert_eq!(ResultType::ErrUnknownReq.is_error(), true);
        assert_eq!(ResultType::ErrNotSupported.is_error(), true);
        assert_eq!(ResultType::ErrCRCInvld.is_error(), true);
        assert_eq!(ResultType::ErrPageFull.is_error(), true);
        assert_eq!(ResultType::ErrInvldArg.is_error(), true);
    }

    #[test]
    fn msg_data_new() {
        assert_eq!(*MsgData::new().get_array(), [0; 4]);
    }

    #[test]
    fn msg_data_from_array() {
        assert_eq!(
            *MsgData::from_array(&[1, 2, 3, 4]).get_array(),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn msg_data_from_word() {
        assert_eq!(*MsgData::from_word(0x01020304).get_array(), [4, 3, 2, 1]);
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
            ResultType::Ok,
            5,
            &MsgData::from_array(&[0x01, 0x02, 0x03, 0x04]),
        );
        assert_eq!(msg.request, RequestType::Ping);
        assert_eq!(msg.result, ResultType::Ok);
        assert_eq!(msg.packet_id, 5);
        assert_eq!(*msg.data.get_array(), [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn msg_from_raw_data_array() {
        let msg = Msg::from_raw_data_array(&[0x03, 0x01, 0x01, 0x05, 0x01, 0x02, 0x03, 0x04]);
        assert_eq!(msg.request, RequestType::DevInfoVID);
        assert_eq!(msg.result, ResultType::Ok);
        assert_eq!(msg.packet_id, 5);
        assert_eq!(*msg.data.get_array(), [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn msg_to_raw_data_array() {
        let msg = Msg::new(
            RequestType::DevInfoVID,
            ResultType::Ok,
            5,
            &MsgData::from_array(&[0x01, 0x02, 0x03, 0x04]),
        );
        assert_eq!(
            msg.to_raw_data_array(),
            [0x03, 0x01, 0x01, 0x05, 0x01, 0x02, 0x03, 0x04]
        );
    }

    #[test]
    fn msg_new_std_request() {
        let msg = Msg::new_std_request(RequestType::Ping);
        assert_eq!(msg.request, RequestType::Ping);
        assert_eq!(msg.result, ResultType::None);
        assert_eq!(msg.packet_id, 0);
        assert_eq!(*msg.data.get_array(), [0; 4]);
    }

    #[test]
    fn msg_is_response_ok_ok() {
        let request = Msg::new_std_request(RequestType::Ping);

        let response = Msg::new(
            RequestType::Ping,
            ResultType::Ok,
            0,
            &MsgData::from_array(&[0x01, 0x02, 0x03, 0x04]),
        );

        assert_eq!(request.is_response_ok(&response), Ok(()));
    }

    #[test]
    fn msg_is_response_ok_error_result() {
        let request = Msg::new_std_request(RequestType::Ping);

        let response = Msg::new(
            RequestType::Ping,
            ResultType::ErrNotSupported,
            0,
            &MsgData::new(),
        );

        match request.is_response_ok(&response) {
            Ok(_) => panic!("Response error expected!"),
            Err(e) => match e {
                Error::ResultError(_) => assert!(true),
                _ => panic!("Unexpected error type!"),
            },
        }
    }

    #[test]
    fn msg_is_response_ok_msg_corrupted_packet_id() {
        let request = Msg::new_std_request(RequestType::Ping);
        let response = Msg::new(RequestType::Ping, ResultType::Ok, 13, &MsgData::new());

        match request.is_response_ok(&response) {
            Ok(_) => panic!("Response error expected!"),
            Err(e) => match e {
                Error::MsgCorruption(_) => assert!(true),
                _ => panic!("Unexpected error type!"),
            },
        }
    }

    #[test]
    fn msg_is_response_ok_msg_corrupted_request() {
        let request = Msg::new_std_request(RequestType::Ping);
        let response = Msg::new(RequestType::DevInfoUID, ResultType::Ok, 0, &MsgData::new());

        match request.is_response_ok(&response) {
            Ok(_) => panic!("Response error expected!"),
            Err(e) => match e {
                Error::MsgCorruption(_) => assert!(true),
                _ => panic!("Unexpected error type!"),
            },
        }
    }

    #[test]
    fn msg_is_response_data_ok() {
        let mut request = Msg::new_std_request(RequestType::Ping);
        let mut response = Msg::new_std_request(RequestType::Ping);

        request.data = MsgData::from_word(0xDEADBEEF);
        response.data = MsgData::from_word(0xDEADBEEF);

        assert!(request.is_response_data_ok(&response).is_ok());
    }

    #[test]
    fn msg_is_response_data_ok_err() {
        let mut request = Msg::new_std_request(RequestType::Ping);
        let mut response = Msg::new_std_request(RequestType::Ping);

        request.data = MsgData::from_word(0xDEADBEEF);
        response.data = MsgData::from_word(0xDEADBEEA);

        assert!(request.is_response_data_ok(&response).is_err());
    }
}
