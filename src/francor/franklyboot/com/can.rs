use socketcan::{CANFilter, CANFrame, CANSocket};
use std::time::Duration;

use crate::francor::franklyboot::{
    com::{
        msg::{Msg, RequestType},
        ComInterface, ComMode,
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
    socket: CANSocket,

    /// Timeout for receiving messages
    timeout: Duration,
}

impl CANInterface {
    ///
    /// Open serial port
    ///
    /// This function opens the serial port with the given name and the given baud rate.
    ///
    /// # Arguments
    ///
    /// * `port_name` - Name of the serial port
    ///
    pub fn open(port_name: &str) -> Result<CANInterface, Error> {
        let socket = CANSocket::open(port_name);
        match socket {
            Ok(socket) => {
                socket
                    .set_read_timeout(CAN_RX_TIMEOUT)
                    .expect("Failed to set rx timeout!");

                // clear rx messages
                loop {
                    match socket.read_frame() {
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }

                Ok(CANInterface {
                    socket,
                    timeout: CAN_RX_TIMEOUT,
                })
            }
            Err(e) => Err(Error::Error(format!(
                "Error opening socket for \"{}\": \"{}\"",
                port_name, e
            ))),
        }
    }

    ///
    /// Opens an interface and sends a ping to the network to search for devices
    ///
    /// Opens an interface and sends a broadcast ping message to the network.
    /// All responding nodes will be added to the result vector.
    pub fn ping_network(port_name: &str) -> Result<Vec<u8>, Error> {
        // Open interface
        let mut interface = Self::open(port_name)?;

        // Config interface to broadcast
        interface.set_mode(ComMode::Broadcast)?;

        // Send ping
        let ping_request = Msg::new_std_request(RequestType::Ping);
        interface.send(&ping_request)?;

        // Receive until no new response
        // Store node ids
        let mut node_id_lst = Vec::new();
        loop {
            match interface.socket.read_frame() {
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
    fn set_mode(&mut self, mode: ComMode) -> Result<(), Error> {
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
            Ok(filter) => match self.socket.set_filter(&[filter]) {
                Ok(_) => Ok(()),
                Err(e) => Err(Error::Error(format!("Failed to set filter: {}", e))),
            },
            Err(e) => {
                return Err(Error::Error(format!("Error config filter: \"{}\"", e)));
            }
        }
    }

    fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<(), Error> {
        match self.socket.set_read_timeout(timeout) {
            Ok(_) => {
                self.timeout = timeout;

                Ok(())
            }
            Err(e) => Err(Error::Error(format!("Failed to set timeout: {}", e))),
        }
    }

    fn get_timeout(&self) -> std::time::Duration {
        self.timeout
    }

    fn send(&mut self, msg: &Msg) -> Result<(), Error> {
        let frame = CANFrame::new(CAN_BROADCAST_ID, &msg.to_raw_data_array(), false, false)
            .map_err(|e| Error::Error(format!("{}", e)))?;

        self.socket
            .write_frame(&frame)
            .map_err(|e| Error::Error(format!("{}", e)))?;

        Ok(())
    }

    fn recv(&mut self) -> Result<Msg, Error> {
        match self.socket.read_frame() {
            Ok(frame) => {
                return Ok(Self::can_frame_to_msg(&frame));
            }
            // Message timeout
            Err(_) => {}
        }

        return Err(Error::ComNoResponse);
    }
}
