use socketcan::{CANFilter, CANFrame, CANSocket};
use std::time::Duration;

use crate::francor::franklyboot::{
    com::{msg::Msg, ComInterface, ComMode},
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
}

impl ComInterface for CANInterface {
    fn set_mode(&mut self, _mode: ComMode) -> Result<(), Error> {
        Err(Error::NotSupported)
    }

    fn set_timeout(&mut self, _timeout: std::time::Duration) -> Result<(), Error> {
        Err(Error::NotSupported)
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
                let data = [
                    frame.data()[0],
                    frame.data()[1],
                    frame.data()[2],
                    frame.data()[3],
                    frame.data()[4],
                    frame.data()[5],
                    frame.data()[6],
                    frame.data()[7],
                ];

                return Ok(Msg::from_raw_data_array(&data));
            }
            // Message timeout
            Err(_) => {}
        }

        return Err(Error::ComNoResponse);
    }
}
