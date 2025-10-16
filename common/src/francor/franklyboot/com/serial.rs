use serialport::SerialPort;
use std::time::Duration;

use crate::francor::franklyboot::{
    com::{msg::Msg, ComConnParams, ComInterface, ComMode},
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
    port: Option<Box<dyn SerialPort>>,

    /// Timeout for receiving messages
    timeout: Duration,
}

impl ComInterface for SerialInterface {
    fn create() -> Result<Self, Error> {
        Ok(SerialInterface {
            port: None,
            timeout: RX_TIMEOUT,
        })
    }

    fn open(&mut self, params: &ComConnParams) -> Result<(), Error> {
        if params.name.is_none() {
            return Err(Error::Error("Serial port name not set!".to_string()));
        }

        if params.baud_rate.is_none() {
            return Err(Error::Error("Serial port baud rate not set!".to_string()));
        }

        let port = serialport::new(params.name.clone().unwrap(), params.baud_rate.unwrap())
            .timeout(RX_TIMEOUT)
            .open()
            .map_err(|e| Error::Error(format!("Failed to open serial port: {}", e)))?;

        self.port = Some(port);

        Ok(())
    }

    fn is_network() -> bool {
        false
    }

    fn scan_network(&mut self) -> Result<Vec<u8>, Error> {
        Err(Error::NotSupported)
    }

    fn set_mode(&mut self, _mode: ComMode) -> Result<(), Error> {
        Err(Error::NotSupported)
    }

    fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<(), Error> {
        match self.port.as_mut() {
            Some(port) => {
                port.set_timeout(timeout)
                    .map_err(|e| Error::Error(format!("Failed to set timeout: {}", e)))?;
                self.timeout = timeout;
                Ok(())
            }
            None => Err(Error::Error("Serial port not open!".to_string())),
        }
    }

    fn get_timeout(&self) -> std::time::Duration {
        self.timeout
    }

    fn send(&mut self, msg: &Msg) -> Result<(), Error> {
        match self.port.as_mut() {
            Some(port) => {
                port.clear(serialport::ClearBuffer::All).map_err(|e| {
                    Error::Error(format!("Failed to clear serial port buffers! {}", e))
                })?;

                port.write_all(&msg.to_raw_data_array())
                    .map_err(|e| Error::Error(format!("Failed to write to serial port: {}", e)))?;

                Ok(())
            }
            None => Err(Error::Error("Serial port not open!".to_string())),
        }
    }

    fn recv(&mut self) -> Result<Msg, Error> {
        match self.port.as_mut() {
            Some(port) => {
                let mut data = [0u8; 8];
                port.read_exact(&mut data)
                    .map_err(|e| Error::Error(format!("Failed to read from serial port: {}", e)))?;

                Ok(Msg::from_raw_data_array(&data))
            }
            None => Err(Error::Error("Serial port not open!".to_string())),
        }
    }
}
