use crate::francor::franklyboot::{
    com::{
        msg::{Msg, MsgData, RequestType},
        ComInterface,
    },
    Error,
};

use std::fmt;

// EntryType --------------------------------------------------------------------------------------

/// Enumeration defining the different entry types of the device
///
/// This enumeration defines the different entry types of the device. The entry types are:
/// - Const: Constant value which can not be changed (read only and will never change)
/// - RO: Read only value which can be read but not changed
/// - RW: Read and write value which can be read and changed
/// - CMD: Command which can be executed
#[derive(Debug, PartialEq)]
pub enum EntryType {
    Const,
    RO,
    RW,
    Cmd,
}

impl EntryType {
    /// Check if entry is a constant value
    pub fn is_const(&self) -> bool {
        match self {
            EntryType::Const => true,
            EntryType::RO => false,
            EntryType::RW => false,
            EntryType::Cmd => false,
        }
    }

    /// Check if entry is a read only value
    pub fn is_readable(&self) -> bool {
        match self {
            EntryType::Const => true,
            EntryType::RO => true,
            EntryType::RW => true,
            EntryType::Cmd => false,
        }
    }

    /// Check if entry is writeable
    pub fn is_writeable(&self) -> bool {
        match self {
            EntryType::Const => false,
            EntryType::RO => false,
            EntryType::RW => true,
            EntryType::Cmd => false,
        }
    }

    /// Check if entry is executable
    pub fn is_executable(&self) -> bool {
        match self {
            EntryType::Const => false,
            EntryType::RO => false,
            EntryType::RW => false,
            EntryType::Cmd => true,
        }
    }
}

/// Implementation of the Display trait for the EntryType enumeration
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

/// Entry -----------------------------------------------------------------------------------------

/// Representation of an device entry
///
/// This struct represents an entry of a device. An entry is a value which can be read, written or
/// executed. The entry has a name, a type and a value. The value can be None if the entry is not
/// readable (exec only) or if the entry is not initialized.
pub struct Entry {
    entry_type: EntryType,
    name: String,
    request_type: RequestType,
    value: Option<MsgData>,
}

impl Entry {
    /// Create a new entry
    pub fn new(entry_type: EntryType, name: &str, request_type: RequestType) -> Self {
        Entry {
            entry_type: entry_type,
            name: name.to_string(),
            request_type: request_type,
            value: None,
        }
    }

    /// Get the type of the entry
    pub fn get_entry_type(&self) -> &EntryType {
        &self.entry_type
    }

    /// Get the name of the entry
    pub fn get_name(&self) -> &String {
        &self.name
    }

    /// Get the message request type of the entry
    pub fn get_request_type(&self) -> &RequestType {
        &self.request_type
    }

    /// Get the value of the entry (MsgData)
    pub fn get_value(&self) -> &Option<MsgData> {
        &self.value
    }

    /// Read the value of the entry (buffered or from device)
    ///
    /// This function reads the value of the entry. If the entry is a constant value the value will
    /// be read from the device only once and then buffered. If the entry is not a constant value
    /// the value will be read from the device every time this function is called.
    ///
    pub fn read_value<T: ComInterface>(&mut self, interface: &mut T) -> Result<&MsgData, Error> {
        if self.entry_type.is_readable() {
            if self.entry_type.is_const() && self.value.is_none() {
                self._read_from_device(interface)?;
            }

            Ok(&self.value.as_ref().unwrap())
        } else {
            Err(Error::Error(format!(
                "Device entry \"{}\" of type {} is not readable!",
                self.name, self.entry_type
            )))
        }
    }

    fn _read_from_device<T: ComInterface>(&mut self, interface: &mut T) -> Result<(), Error> {
        let request = Msg::new_std_request(self.request_type);

        interface.send(&request)?;
        let response = interface.recv()?;
        request.is_response_ok(&response)?;

        self.value = Some(response.get_data().clone());

        Ok(())
    }

    /*
        pub fn write_value<T: ComInterface>(
            &mut self,
            interface: &mut T,
            data: &MsgData,
        ) -> Result<bool, Error> {
            if self.entry_type.is_writeable() {
                self._write_to_device(interface, data)
            } else {
                Err(Error::Error(format!(
                    "Device entry \"{}\" of type {} is not writeable!",
                    self.name, self.entry_type
                )))
            }
        }

        pub fn exec<T: ComInterface>(&mut self, interface: &mut T) -> Result<bool, Error> {
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
                        Err(Error::Error(format!(
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
    */
}

// Tests ------------------------------------------------------------------------------------------
