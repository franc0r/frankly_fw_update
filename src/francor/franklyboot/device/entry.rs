use crate::francor::franklyboot::com::{
    msg::{Msg, MsgData, RequestType, ResponseType},
    ComError, ComInterface,
};

use std::fmt;

#[derive(Debug, PartialEq)]
pub enum EntryType {
    Const,
    RO,
    RW,
    Cmd,
}

impl EntryType {
    pub fn is_const(&self) -> bool {
        match self {
            EntryType::Const => true,
            EntryType::RO => false,
            EntryType::RW => false,
            EntryType::Cmd => false,
        }
    }

    pub fn is_readable(&self) -> bool {
        match self {
            EntryType::Const => true,
            EntryType::RO => true,
            EntryType::RW => true,
            EntryType::Cmd => false,
        }
    }

    pub fn is_writeable(&self) -> bool {
        match self {
            EntryType::Const => false,
            EntryType::RO => false,
            EntryType::RW => true,
            EntryType::Cmd => false,
        }
    }

    pub fn is_executable(&self) -> bool {
        match self {
            EntryType::Const => false,
            EntryType::RO => false,
            EntryType::RW => false,
            EntryType::Cmd => true,
        }
    }
}

impl fmt::Display for EntryType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EntryType::Const => write!(f, "Const"),
            EntryType::RO => write!(f, "RO"),
            EntryType::RW => write!(f, "RW"),
            EntryType::Cmd => write!(f, "CMD"),
        }
    }
}

pub struct Entry {
    entry_type: EntryType,
    name: String,
    request_type: RequestType,
    value: Option<MsgData>,
}

impl Entry {
    pub fn new(entry_type: EntryType, name: &str, request_type: RequestType) -> Self {
        Entry {
            entry_type: entry_type,
            name: name.to_string(),
            request_type: request_type,
            value: None,
        }
    }

    pub fn get_entry_type(&self) -> &EntryType {
        &self.entry_type
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_request_type(&self) -> &RequestType {
        &self.request_type
    }

    pub fn get_value(&self) -> &Option<MsgData> {
        &self.value
    }

    pub fn read_value<T: ComInterface>(
        &mut self,
        interface: &mut T,
    ) -> Result<Option<MsgData>, ComError> {
        if self.entry_type.is_readable() {
            if self.entry_type.is_const() {
                if self.value.is_none() {
                    self._read_from_device(interface)
                } else {
                    Ok(self.value)
                }
            } else {
                self._read_from_device(interface)
            }
        } else {
            Err(ComError::Error(format!(
                "Device entry \"{}\" of type {} is not readable!",
                self.name, self.entry_type
            )))
        }
    }

    pub fn write_value<T: ComInterface>(
        &mut self,
        interface: &mut T,
        data: &MsgData,
    ) -> Result<bool, ComError> {
        if self.entry_type.is_writeable() {
            self._write_to_device(interface, data)
        } else {
            Err(ComError::Error(format!(
                "Device entry \"{}\" of type {} is not writeable!",
                self.name, self.entry_type
            )))
        }
    }

    pub fn exec<T: ComInterface>(&mut self, interface: &mut T) -> Result<bool, ComError> {
        let request = Msg::new_std_request(self.request_type);

        interface.send(&request)?;

        match interface.recv()? {
            Some(response) => {
                let request_valid = response.get_request() == request.get_request();
                let response_valid = response.get_response() == ResponseType::RespAck;
                let packet_id_valid = response.get_packet_id() == request.get_packet_id();
                let msg_valid = request_valid && response_valid && packet_id_valid;

                if msg_valid {
                    self.value = Some(response.get_data().clone());
                    Ok(true)
                } else {
                    Err(ComError::Error(format!(
                        "Invalid response from device!\n\
                        Read device entry \"{}\"\n\
                        Tx: {:#?} {}\n\
                        Rx: {:#?} {:#?} {}",
                        self.name,
                        request.get_request(),
                        request.get_packet_id(),
                        response.get_request(),
                        response.get_response(),
                        response.get_packet_id()
                    )))
                }
            }
            None => Ok(false),
        }
    }

    fn _read_from_device<T: ComInterface>(
        &mut self,
        interface: &mut T,
    ) -> Result<Option<MsgData>, ComError> {
        let request = Msg::new_std_request(self.request_type);

        interface.send(&request)?;

        match interface.recv()? {
            Some(response) => {
                let request_valid = response.get_request() == request.get_request();
                let response_valid = response.get_response() == ResponseType::RespAck;
                let packet_id_valid = response.get_packet_id() == request.get_packet_id();
                let msg_valid = request_valid && response_valid && packet_id_valid;

                if msg_valid {
                    self.value = Some(response.get_data().clone());
                    Ok(self.value)
                } else {
                    Err(ComError::Error(format!(
                        "Invalid response from device!\n\
                        Read device entry \"{}\"\n\
                        Tx: {:#?} {}\n\
                        Rx: {:#?} {:#?} {}",
                        self.name,
                        request.get_request(),
                        request.get_packet_id(),
                        response.get_request(),
                        response.get_response(),
                        response.get_packet_id()
                    )))
                }
            }
            None => Ok(None),
        }
    }

    fn _write_to_device<T: ComInterface>(
        &mut self,
        interface: &mut T,
        data: &MsgData,
    ) -> Result<bool, ComError> {
        let request = Msg::new(self.request_type, ResponseType::RespNone, 0, data);

        interface.send(&request)?;

        match interface.recv()? {
            Some(response) => {
                let request_valid = response.get_request() == request.get_request();
                let response_valid = response.get_response() == ResponseType::RespAck;
                let packet_id_valid = response.get_packet_id() == request.get_packet_id();
                let data_valid = response.get_data() == request.get_data();
                let msg_valid = request_valid && response_valid && packet_id_valid && data_valid;

                if msg_valid {
                    self.value = Some(response.get_data().clone());
                    Ok(true)
                } else {
                    Err(ComError::Error(format!(
                        "Invalid response from device!\n\
                        Write device entry \"{}\"\n\
                        Tx: {:#?} {}\n\
                        Rx: {:#?} {:#?} {}",
                        self.name,
                        request.get_request(),
                        request.get_packet_id(),
                        response.get_request(),
                        response.get_response(),
                        response.get_packet_id()
                    )))
                }
            }
            None => Ok(false),
        }
    }
}
