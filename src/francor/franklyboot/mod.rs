// Defininition of modules ------------------------------------------------------------------------

pub mod com;
pub mod device;
pub mod firmware;
pub mod flash;

use std::fmt;

// Error ------------------------------------------------------------------------------------------

///
/// Franklyboot error enumeration.
///
/// This enumeration specifies the different errors which can occur with the frankly bootloader api.
/// Most enumeration contains a string for error description.
///
#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    /// No response from device
    ComNoResponse,

    /// Communication driver error with description
    ComError(String),

    /// Response message contains an error result
    ResultError(String),

    /// Message corruption: Response received from device but data seems corrupted
    MsgCorruption(String),

    /// Function not supported/implemented
    NotSupported,

    /// General error
    Error(String),
}

/// Implementation of the Display trait for the Error enumeration
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::ComNoResponse => {
                write!(f, "ComNoResponse: No response from device")
            }
            Error::ComError(desc) => {
                write!(f, "ComError: {}", desc)
            }
            Error::ResultError(desc) => {
                write!(f, "ResultError: {}", desc)
            }
            Error::MsgCorruption(desc) => {
                write!(f, "MsgCorruption: {}", desc)
            }
            Error::NotSupported => {
                write!(f, "NotSupported: Command is not supported/implemented")
            }
            Error::Error(desc) => {
                write!(f, "Error: {}", desc)
            }
        }
    }
}
