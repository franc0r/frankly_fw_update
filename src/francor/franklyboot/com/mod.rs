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

// Com Interface Trait -----------------------------------------------------------------------------

pub trait ComInterface {
    fn set_filter(&mut self, node_id: u8) -> Result<(), ComError>;
    fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<(), ComError>;
    fn get_timeout(&self) -> std::time::Duration;
    fn send(&mut self, msg: &Msg) -> Result<(), ComError>;
    fn recv(&mut self) -> Result<Option<Msg>, ComError>;
}

// Com Simulator for Testing -----------------------------------------------------------------------

pub struct ComSimulator {
    response_queue: Vec<Msg>,
    send_error: Option<ComError>,
    recv_error: Option<ComError>,
    recv_timeout: bool,
}

impl ComSimulator {
    pub fn new() -> Self {
        ComSimulator {
            response_queue: Vec::new(),
            send_error: None,
            recv_error: None,
            recv_timeout: false,
        }
    }

    pub fn add_response(&mut self, msg: Msg) {
        self.response_queue.push(msg);
    }

    pub fn get_response(&mut self) -> Option<Msg> {
        if self.response_queue.is_empty() {
            None
        } else {
            Some(self.response_queue.remove(0))
        }
    }

    pub fn set_send_error(&mut self, error: ComError) {
        self.send_error = Some(error);
    }

    pub fn set_recv_error(&mut self, error: ComError) {
        self.recv_error = Some(error);
    }

    pub fn set_recv_timeout_error(&mut self) {
        self.recv_timeout = true;
    }
}

impl ComInterface for ComSimulator {
    fn set_filter(&mut self, _node_id: u8) -> Result<(), ComError> {
        Ok(())
    }

    fn set_timeout(&mut self, _timeout: std::time::Duration) -> Result<(), ComError> {
        Ok(())
    }

    fn get_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(0)
    }

    fn send(&mut self, _msg: &Msg) -> Result<(), ComError> {
        if self.send_error.is_some() {
            let error = self.send_error.clone().unwrap();
            self.send_error = None;
            Err(error)
        } else {
            Ok(())
        }
    }

    fn recv(&mut self) -> Result<Option<Msg>, ComError> {
        if self.recv_error.is_some() {
            let error = self.recv_error.clone().unwrap();
            self.recv_error = None;
            Err(error)
        } else {
            if self.recv_timeout {
                self.recv_timeout = false;
                Ok(None)
            } else {
                Ok(self.get_response())
            }
        }
    }
}
