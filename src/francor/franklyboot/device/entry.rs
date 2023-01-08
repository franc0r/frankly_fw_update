use crate::francor::franklyboot::{
    com::{
        msg::{Msg, MsgData, RequestType, ResultType},
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
    request_type: RequestType,
    value: Option<MsgData>,
}

impl Entry {
    /// Create a new entry
    pub fn new(entry_type: EntryType, request_type: RequestType) -> Self {
        Entry {
            entry_type: entry_type,
            request_type: request_type,
            value: None,
        }
    }

    /// Get the type of the entry
    pub fn get_entry_type(&self) -> &EntryType {
        &self.entry_type
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
            let read_const_value = self.entry_type.is_const() && self.value.is_none();
            let read_normal_value = self.entry_type.is_readable();

            if read_const_value || read_normal_value {
                self._read_from_device(interface)?;
            }

            Ok(&self.value.as_ref().unwrap())
        } else {
            Err(Error::Error(format!(
                "Device entry of type {} is not readable!",
                self.entry_type
            )))
        }
    }

    /// Write the value of the entry to the device
    ///
    /// This function writes the value of the entry to the device. The entry must be of type RW.
    ///
    pub fn write_value<T: ComInterface>(
        &mut self,
        interface: &mut T,
        packet_id: u8,
        data: &MsgData,
    ) -> Result<(), Error> {
        if self.entry_type.is_writeable() {
            self._write_to_device(interface, packet_id, data)
        } else {
            Err(Error::Error(format!(
                "Device entry of type {} is not writeable!",
                self.entry_type
            )))
        }
    }

    /// Execute entry
    ///
    /// This function executes the entry. The entry must be of type CMD.
    ///
    pub fn exec<T: ComInterface>(&mut self, interface: &mut T, argument: u32) -> Result<(), Error> {
        if self.entry_type.is_executable() {
            self._write_to_device(interface, 0, &MsgData::from_word(argument))
        } else {
            Err(Error::Error(format!(
                "Device entry of type {} is not executable!",
                self.entry_type
            )))
        }
    }

    // Private functions --------------------------------------------------------------------------

    fn _read_from_device<T: ComInterface>(&mut self, interface: &mut T) -> Result<(), Error> {
        let request = Msg::new_std_request(self.request_type);

        interface.send(&request)?;
        let response = interface.recv()?;
        request.is_response_ok(&response)?;

        self.value = Some(response.get_data().clone());

        Ok(())
    }

    fn _write_to_device<T: ComInterface>(
        &mut self,
        interface: &mut T,
        packet_id: u8,
        data: &MsgData,
    ) -> Result<(), Error> {
        let request = Msg::new(self.request_type, ResultType::None, packet_id, data);

        interface.send(&request)?;
        let response = interface.recv()?;
        request.is_response_ok(&response)?;
        request.is_response_data_ok(&response)?;

        self.value = Some(response.get_data().clone());

        Ok(())
    }
}

// Tests ------------------------------------------------------------------------------------------
