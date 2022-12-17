pub mod msg;

use msg::Msg;

// Node ID ----------------------------------------------------------------------------------------

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum NodeID {
    None,
    Broadcast,
    Specific(u8),
}

// Com Error --------------------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone)]
pub enum ComError {
    // Error with message content
    MsgError(String),

    // Error with message driver
    Error(String),
}

pub trait ComInterface {
    fn set_filter(&mut self, node_id: u8) -> Result<(), ComError>;
    fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<(), ComError>;
    fn get_timeout(&self) -> std::time::Duration;
    fn send(&mut self, msg: &Msg) -> Result<(), ComError>;
    fn recv(&mut self) -> Result<Option<Msg>, ComError>;
}
