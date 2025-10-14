use socketcan::{CanFilter, CanFrame, CanSocket, EmbeddedFrame, Frame, Socket, SocketOptions, StandardId};
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
    socket: Option<CanSocket>,

    /// Timeout for receiving messages
    timeout: Duration,
}

impl CANInterface {
    // Private functions --------------------------------------------------------------------------

    fn can_frame_to_msg(can_frame: &socketcan::frame::CanDataFrame) -> Msg {
        let data = can_frame.data();
        let msg_data = [
            data[0],
            data[1],
            data[2],
            data[3],
            data[4],
            data[5],
            data[6],
            data[7],
        ];

        return Msg::from_raw_data_array(&msg_data);
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

        let socket = CanSocket::open(params.name.clone().unwrap().as_str());
        match socket {
            Ok(socket) => {
                socket
                    .set_read_timeout(Some(self.timeout))
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
                        Ok(frame) => {
                            // Only process data frames
                            match frame {
                                CanFrame::Data(can_frame) => {
                                    let response = Self::can_frame_to_msg(&can_frame);
                                    if ping_request.is_response_ok(&response).is_ok() {
                                        let raw_id = can_frame.raw_id();
                                        let node_id = ((raw_id - CAN_BASE_ID) / 2) as u8;
                                        node_id_lst.push(node_id);
                                    }
                                }
                                _ => {} // Ignore non-data frames
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
                let filter = CanFilter::new(can_rx_msg_id, can_rx_msg_mask);
                match socket.set_filters(&[filter]) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(Error::Error(format!("Failed to set filter: {}", e))),
                }
            }
            None => Err(Error::Error(format!("CAN socket not open!"))),
        }
    }

    fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<(), Error> {
        match self.socket.as_mut() {
            Some(socket) => match socket.set_read_timeout(Some(timeout)) {
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
                let id = StandardId::new(CAN_BROADCAST_ID as u16)
                    .ok_or_else(|| Error::Error(format!("Invalid CAN ID: {}", CAN_BROADCAST_ID)))?;
                let frame = socketcan::frame::CanDataFrame::new(id, &msg.to_raw_data_array())
                    .ok_or_else(|| Error::Error(format!("Failed to create CAN data frame")))?;

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
                        // Only process data frames
                        match frame {
                            CanFrame::Data(can_frame) => {
                                return Ok(Self::can_frame_to_msg(&can_frame));
                            }
                            _ => {
                                // Non-data frames are ignored, try again
                                return Err(Error::ComNoResponse);
                            }
                        }
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
