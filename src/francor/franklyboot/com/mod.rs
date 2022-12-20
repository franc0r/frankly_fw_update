pub mod msg;

use msg::{Msg, MsgData, ResponseType};
use std::collections::VecDeque;

use self::msg::RequestType;

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

pub fn handle_read_data_request<T: ComInterface>(
    interface: &mut T,
    request: &Msg,
) -> Result<Option<MsgData>, ComError> {
    interface.send(request)?;

    match interface.recv()? {
        Some(response) => {
            let request_valid = request.get_request() == response.get_request();
            let response_valid = response.get_response() == ResponseType::RespAck;
            let msg_valid = request_valid && response_valid;

            if msg_valid {
                return Ok(Some(response.get_data().clone()));
            } else {
                return Err(ComError::MsgError(format!(
                    "Device response is invalid! \
                     TX: Request {:?}\n\tRX: RequestType {:?} ResponseType {:?}",
                    request.get_request(),
                    response.get_request(),
                    response.get_response()
                )));
            }
        }
        None => {
            return Ok(None);
        }
    }
}

pub fn hande_write_request<T: ComInterface>(
    interface: &mut T,
    request_type: RequestType,
    packet_id: u8,
    data: &MsgData,
) -> Result<bool, ComError> {
    let request = Msg::new(request_type, ResponseType::RespNone, packet_id, data);

    interface.send(&request)?;

    match interface.recv()? {
        Some(response) => {
            let request_valid = request.get_request() == response.get_request();
            let response_valid = response.get_response() == ResponseType::RespAck;
            let packet_id_valid = response.get_packet_id() == request.get_packet_id();
            let data_valid = response.get_data() == request.get_data();
            let msg_valid = request_valid && response_valid && packet_id_valid && data_valid;

            if msg_valid {
                return Ok(true);
            } else {
                return Err(ComError::MsgError(format!(
                    "Write data request message error!\n\
                    Tx: Request: {:#?} Packet-ID: {}, Data: {}\n\
                    Rx: Request: {:#?} Reponse: {:#?} Packet-ID: {}, Data: {}\n",
                    request.get_request(),
                    request.get_packet_id(),
                    request.get_data().to_word(),
                    response.get_request(),
                    response.get_response(),
                    request.get_packet_id(),
                    request.get_data().to_word()
                )));
            }
        }

        None => {
            return Ok(false);
        }
    }
}

pub fn handle_command_request<T: ComInterface>(
    interface: &mut T,
    request: &Msg,
) -> Result<bool, ComError> {
    interface.send(request)?;

    match interface.recv()? {
        Some(response) => {
            let request_valid = request.get_request() == response.get_request();
            let response_valid = response.get_response() == ResponseType::RespAck;
            let msg_valid = request_valid && response_valid;

            if msg_valid {
                return Ok(true);
            } else {
                return Err(ComError::MsgError(format!(
                    "Device response is invalid! \
                     TX: Request {:?}\n\tRX: RequestType {:?} ResponseType {:?}",
                    request.get_request(),
                    response.get_request(),
                    response.get_response()
                )));
            }
        }
        None => {
            return Ok(false);
        }
    }
}

// Com Simulator for Testing -----------------------------------------------------------------------

pub struct ComSimulator {
    response_queue: VecDeque<Msg>,
    send_error: Option<ComError>,
    recv_error: Option<ComError>,
    recv_timeout: bool,
}

impl ComSimulator {
    pub fn new() -> Self {
        ComSimulator {
            response_queue: VecDeque::new(),
            send_error: None,
            recv_error: None,
            recv_timeout: false,
        }
    }

    pub fn add_response(&mut self, msg: Msg) {
        self.response_queue.push_back(msg);
    }

    pub fn get_response(&mut self) -> Option<Msg> {
        self.response_queue.pop_front()
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
