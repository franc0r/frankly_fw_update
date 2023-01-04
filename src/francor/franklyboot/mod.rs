// Defininition of modules ------------------------------------------------------------------------

pub mod com;
pub mod device;
pub mod firmware;

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

    /// General error
    Error(String),
}
