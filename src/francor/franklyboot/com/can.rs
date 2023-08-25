use socketcan::{CANFilter, CANFrame, CANSocket};
use std::time::Duration;

use crate::francor::franklyboot::{
    com::{
        msg::{Msg, RequestType},
        ComConnParams, ComInterface, ComMode,
    },
    Error,
};

// CAN Interface ----------------------------------------------------------------------------------

pub const CAN_BASE_ID: u32 = 0x781;
pub const CAN_BROADCAST_ID: u32 = 0x780;
pub const CAN_MAX_ID: u32 = 0x7FF;
pub const CAN_RX_TIMEOUT: std::time::Duration = Duration::from_millis(500);

///
/// CAN interface
///
/// This struct implements the communication interface for can communication.
///
pub struct CANInterface {
    /// CAN socket
    socket: Option<CANSocket>,

    /// Timeout for receiving messages
    timeout: Duration,
}

impl CANInterface {
    // Private functions --------------------------------------------------------------------------

    fn can_frame_to_msg(can_frame: &CANFrame) -> Msg {
        let data = [
            can_frame.data()[0],
            can_frame.data()[1],
            can_frame.data()[2],
            can_frame.data()[3],
            can_frame.data()[4],
            can_frame.data()[5],
            can_frame.data()[6],
            can_frame.data()[7],
        ];

        return Msg::from_raw_data_array(&data);
    }
}

impl ComInterface for CANInterface {
    fn create() -> Result<Self, Error> {
        Ok(CANInterface {
            socket: None,
            timeout: CAN_RX_TIMEOUT,
        })
    }

    fn open(&mut self, params: &ComConnParams) -> Result<(), Error> {
        if params.name.is_none() {
            return Err(Error::Error(format!("Serial port name not set!")));
        }

        let socket = CANSocket::open(params.name.clone().unwrap().as_str());
        match socket {
            Ok(socket) => {
                socket
                    .set_read_timeout(self.timeout)
                    .map_err(|_e| Error::Error(format!("Failed to set rx timeout!")))?;

                // clear rx messages
                loop {
                    match socket.read_frame() {
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }

                self.socket = Some(socket);

                Ok(())
            }
            Err(e) => Err(Error::Error(format!(
                "Error opening socket for \"{}\": \"{}\"",
                params.name.clone().unwrap(),
                e
            ))),
        }
    }

    fn is_network() -> bool {
        true
    }

    fn scan_network(&mut self) -> Result<Vec<u8>, Error> {
        // Config interface to broadcast
        self.set_mode(ComMode::Broadcast)?;

        // Send ping
        let ping_request = Msg::new_std_request(RequestType::Ping);
        self.send(&ping_request)?;

        match self.socket.as_mut() {
            Some(socket) => {
                // Receive until no new response
                // Store node ids
                let mut node_id_lst = Vec::new();
                loop {
                    match socket.read_frame() {
                        Ok(can_frame) => {
                            let response = Self::can_frame_to_msg(&can_frame);
                            if ping_request.is_response_ok(&response).is_ok() {
                                let node_id = ((can_frame.id() - CAN_BASE_ID) / 2) as u8;
                                node_id_lst.push(node_id);
                            }
                        }
                        Err(_e) => {
                            break;
                        }
                    }
                }

                Ok(node_id_lst)
            }
            None => Err(Error::Error(format!("CAN socket not open!"))),
        }
    }

    fn set_mode(&mut self, mode: ComMode) -> Result<(), Error> {
        match self.socket.as_mut() {
            Some(socket) => {
                let mut can_rx_msg_id = 0;
                let mut can_rx_msg_mask = 0;

                // Set ID and MASK only if no broadcast is used
                match mode {
                    ComMode::Specific(node_id) => {
                        can_rx_msg_id = CAN_BASE_ID + node_id as u32 * 2 + 1;
                        can_rx_msg_mask = 0x7FF;
                    }
                    _ => {}
                }

                // Set filter
                match CANFilter::new(can_rx_msg_id, can_rx_msg_mask) {
                    Ok(filter) => match socket.set_filter(&[filter]) {
                        Ok(_) => Ok(()),
                        Err(e) => Err(Error::Error(format!("Failed to set filter: {}", e))),
                    },
                    Err(e) => Err(Error::Error(format!("Error config filter: \"{}\"", e))),
                }
            }
            None => Err(Error::Error(format!("CAN socket not open!"))),
        }
    }

    fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<(), Error> {
        match self.socket.as_mut() {
            Some(socket) => match socket.set_read_timeout(timeout) {
                Ok(_) => {
                    self.timeout = timeout;

                    Ok(())
                }
                Err(e) => Err(Error::Error(format!("Failed to set timeout: {}", e))),
            },
            None => Err(Error::Error(format!("CAN socket not open!"))),
        }
    }

    fn get_timeout(&self) -> std::time::Duration {
        self.timeout
    }

    fn send(&mut self, msg: &Msg) -> Result<(), Error> {
        match self.socket.as_mut() {
            Some(socket) => {
                let frame = CANFrame::new(CAN_BROADCAST_ID, &msg.to_raw_data_array(), false, false)
                    .map_err(|e| Error::Error(format!("{}", e)))?;

                socket
                    .write_frame(&frame)
                    .map_err(|e| Error::Error(format!("{}", e)))?;

                Ok(())
            }
            None => Err(Error::Error(format!("CAN socket not open!"))),
        }
    }

    fn recv(&mut self) -> Result<Msg, Error> {
        match self.socket.as_mut() {
            Some(socket) => {
                match socket.read_frame() {
                    Ok(frame) => {
                        return Ok(Self::can_frame_to_msg(&frame));
                    }
                    // Message timeout
                    Err(_) => {}
                }

                return Err(Error::ComNoResponse);
            }
            None => {
                return Err(Error::Error(format!("CAN socket not open!")));
            }
        }
    }
}
