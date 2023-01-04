pub mod msg;

use crate::francor::franklyboot::{com::msg::Msg, Error};

use std::collections::VecDeque;

// ComMode ----------------------------------------------------------------------------------------

/// Communication mode
///
/// This enumeration specifies the supported communication modes by the com interface
///
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ComMode {
    /// Broadcast message to all devices and receive messages from all nodes
    Broadcast,

    /// Send message to specific node
    Specific(u8),
}

// Com Interface Trait -----------------------------------------------------------------------------

/// Interface trait for communication with device
///
/// Standarized interface trait which every communication interface must implement.
/// It enables the communication with the devices and handles the low layer com protocol.
///
pub trait ComInterface {
    /// Set the communication mode (broadcast or specific node)
    ///
    /// Set the communication mode to:
    /// - Broadcast: Send messages to all devices and receive messages from all nodes
    /// - Specific: Send messages to specific node and receive messages from this node
    ///
    fn set_mode(&mut self, mode: ComMode) -> Result<(), Error>;

    /// Set maximum time to wait for a response
    fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<(), Error>;

    /// Get active timeout value
    fn get_timeout(&self) -> std::time::Duration;

    /// Send a message to the device
    fn send(&mut self, msg: &Msg) -> Result<(), Error>;

    /// Receive a message from the device
    ///
    /// This function blocks until a message is received or the timeout is reached.
    fn recv(&mut self) -> Result<Msg, Error>;
}

// Com Simulator for Testing -----------------------------------------------------------------------

pub struct ComSimulator {
    response_queue: VecDeque<Msg>,
    send_error: Option<Error>,
    recv_error: Option<Error>,
}

impl ComSimulator {
    pub fn new() -> Self {
        ComSimulator {
            response_queue: VecDeque::new(),
            send_error: None,
            recv_error: None,
        }
    }

    pub fn add_response(&mut self, msg: Msg) {
        self.response_queue.push_back(msg);
    }

    pub fn get_result(&mut self) -> Option<Msg> {
        self.response_queue.pop_front()
    }

    pub fn set_send_error(&mut self, error: Error) {
        self.send_error = Some(error);
    }

    pub fn set_recv_error(&mut self, error: Error) {
        self.recv_error = Some(error);
    }
}

impl ComInterface for ComSimulator {
    fn set_mode(&mut self, _mode: ComMode) -> Result<(), Error> {
        Ok(())
    }

    fn set_timeout(&mut self, _timeout: std::time::Duration) -> Result<(), Error> {
        Ok(())
    }

    fn get_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(0)
    }

    fn send(&mut self, _msg: &Msg) -> Result<(), Error> {
        if self.send_error.is_some() {
            let error = self.send_error.clone().unwrap();
            self.send_error = None;
            Err(error)
        } else {
            Ok(())
        }
    }

    fn recv(&mut self) -> Result<Msg, Error> {
        if self.recv_error.is_some() {
            let error = self.recv_error.clone().unwrap();
            self.recv_error = None;
            Err(error)
        } else {
            Ok(self.response_queue.pop_front().unwrap())
        }
    }
}
