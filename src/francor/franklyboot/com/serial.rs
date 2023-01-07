use serialport::SerialPort;
use std::time::Duration;

use crate::francor::franklyboot::{
    com::{msg::Msg, ComInterface, ComMode},
    Error,
};

// Serial Interface -------------------------------------------------------------------------------

pub const RX_TIMEOUT: std::time::Duration = Duration::from_millis(500);

///
/// Serial interface
///
/// This struct implements the communication interface for serial communication.
///
pub struct SerialInterface {
    /// Serial port interface trait
    port: Box<dyn SerialPort>,

    /// Timeout for receiving messages
    timeout: Duration,
}

impl SerialInterface {
    ///
    /// Open serial port
    ///
    /// This function opens the serial port with the given name and the given baud rate.
    ///
    /// # Arguments
    ///
    /// * `port_name` - Name of the serial port
    /// * `baud_rate` - Baud rate of the serial port
    ///
    pub fn open(port_name: &str, baud_rate: u32) -> Result<SerialInterface, String> {
        let port = serialport::new(port_name, baud_rate)
            .timeout(RX_TIMEOUT)
            .open()
            .map_err(|e| format!("Failed to open serial port: {}", e))?;

        Ok(SerialInterface {
            port,
            timeout: RX_TIMEOUT,
        })
    }
}

impl ComInterface for SerialInterface {
    fn set_mode(&mut self, _mode: ComMode) -> Result<(), Error> {
        Err(Error::NotSupported)
    }

    fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<(), Error> {
        match self.port.set_timeout(timeout) {
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
        self.port
            .clear(serialport::ClearBuffer::All)
            .map_err(|e| Error::Error(format!("Failed to clear serial port buffers! {}", e)))?;
        self.port
            .write_all(&msg.to_raw_data_array())
            .map_err(|e| Error::Error(format!("Failed to write to serial port: {}", e)))?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Msg, Error> {
        let mut data = [0u8; 8];
        self.port
            .read_exact(&mut data)
            .map_err(|e| Error::Error(format!("Failed to read from serial port: {}", e)))?;

        Ok(Msg::from_raw_data_array(&data))
    }
}
