use crate::francor::franklyboot::msg::Msg;

#[derive(Debug, PartialEq, Clone)]
pub enum ComError {
    NoMessage,
    MsgError(String),
    Error(String),
}

pub trait ComInterface {
    fn set_filter(&mut self, node_id: u8) -> Result<(), ComError>;
    fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<(), ComError>;
    fn get_timeout(&self) -> std::time::Duration;
    fn send(&mut self, msg: &Msg) -> Result<(), ComError>;
    fn recv(&mut self) -> Result<Msg, ComError>;
}