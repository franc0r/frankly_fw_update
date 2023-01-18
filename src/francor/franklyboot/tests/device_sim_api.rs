use crate::francor::franklyboot::{
    com::{
        msg::{Msg, RequestType},
        ComInterface, ComMode,
    },
    Error,
};

// Device Simulator C API -------------------------------------------------------------------------

#[link(name = "franklyboot-device-sim-api", kind = "static")]
extern "C" {
    pub fn SIM_reset();
    pub fn SIM_addDevice(node_id: u8) -> bool;
    pub fn SIM_getDeviceCount() -> u32;
    pub fn SIM_sendBroadcastMsg(msg_ptr: *const u8);
    pub fn SIM_sendNodeMsg(node_id: u8, raw_msg_ptr: *const u8);
    pub fn SIM_updateDevices();
    pub fn SIM_getBroadcastResponseMsg(node_id: *mut u8, raw_msg_ptr: *mut u8) -> bool;
    pub fn SIM_getNodeResponseMsg(node_id: u8, raw_msg_ptr: *mut u8) -> bool;
}

// Device Simulator -------------------------------------------------------------------------------

///
/// Device Simulator API
///
/// This struct implements the C-API for the device simulator.
struct DeviceSimAPI;

impl DeviceSimAPI {
    pub fn new() -> Self {
        DeviceSimAPI {}
    }

    pub fn reset(&self) {
        unsafe { SIM_reset() }
    }

    pub fn add_device(&self, node_id: u8) -> bool {
        unsafe { SIM_addDevice(node_id) }
    }

    pub fn get_device_count(&self) -> u32 {
        unsafe { SIM_getDeviceCount() }
    }

    pub fn send_broadcast_msg(&self, msg: &Msg) {
        let raw_msg = msg.to_raw_data_array();
        unsafe {
            SIM_sendBroadcastMsg(raw_msg.as_ptr());
        }
    }

    pub fn send_node_msg(&self, node_id: u8, msg: &Msg) {
        let raw_msg = msg.to_raw_data_array();
        unsafe {
            SIM_sendNodeMsg(node_id, raw_msg.as_ptr());
        }
    }

    pub fn update_devices(&self) {
        unsafe {
            SIM_updateDevices();
        }
    }

    pub fn get_broadcast_response_msg(&self) -> Option<(u8, Msg)> {
        let mut node_id = [0u8; 1];
        let mut raw_msg = [0u8; 8];
        match unsafe { SIM_getBroadcastResponseMsg(node_id.as_mut_ptr(), raw_msg.as_mut_ptr()) } {
            true => {
                return Some((node_id[0], Msg::from_raw_data_array(&raw_msg)));
            }
            _ => {
                return None;
            }
        }
    }

    pub fn get_node_response_msg(&self, node_id: u8) -> Option<Msg> {
        unsafe {
            let mut raw_msg = [0u8; 8];

            if SIM_getNodeResponseMsg(node_id, raw_msg.as_mut_ptr()) {
                return Some(Msg::from_raw_data_array(&raw_msg));
            } else {
                return None;
            }
        }
    }
}

// Device Simulator COM Interface -----------------------------------------------------------------

pub struct DeviceSimInterface {
    sim_api: DeviceSimAPI,
    mode: ComMode,
}

impl DeviceSimInterface {
    pub fn open() -> Self {
        DeviceSimInterface {
            sim_api: DeviceSimAPI::new(),
            mode: ComMode::Broadcast,
        }
    }

    pub fn ping_network() -> Result<Vec<u8>, Error> {
        let mut interface = Self::open();

        // Config interface to broadcast
        interface.set_mode(ComMode::Broadcast)?;

        // Send ping
        let ping_request = Msg::new_std_request(RequestType::Ping);
        interface.send(&ping_request)?;

        // Receive until timeout is received
        let mut node_id_lst = Vec::new();
        loop {
            match interface.sim_api.get_broadcast_response_msg() {
                Some((node_id, response)) => {
                    if ping_request.is_response_ok(&response).is_ok() {
                        node_id_lst.push(node_id);
                    }
                }
                None => {
                    break;
                }
            }
        }

        Ok(node_id_lst)
    }
}

impl ComInterface for DeviceSimInterface {
    fn set_mode(&mut self, mode: ComMode) -> Result<(), Error> {
        self.mode = mode;

        Ok(())
    }

    fn set_timeout(&mut self, _timeout: std::time::Duration) -> Result<(), Error> {
        Err(Error::NotSupported)
    }

    fn get_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(0)
    }

    fn send(&mut self, msg: &Msg) -> Result<(), Error> {
        match self.mode {
            ComMode::Broadcast => {
                self.sim_api.send_broadcast_msg(msg);
            }
            ComMode::Specific(node_id) => {
                self.sim_api.send_node_msg(node_id, msg);
            }
        }

        self.sim_api.update_devices();

        Ok(())
    }

    fn recv(&mut self) -> Result<Msg, Error> {
        match self.mode {
            ComMode::Specific(node_id) => match self.sim_api.get_node_response_msg(node_id) {
                Some(msg) => Ok(msg),
                None => Err(Error::ComNoResponse),
            },
            _ => Err(Error::NotSupported),
        }
    }
}

// Tests ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::francor::franklyboot::device::Device;

    #[test]
    fn sim_dev_api_tests() {
        sim_dev_api_get_device_count();
        sim_dev_api_add_node();
        sim_dev_api_add_same_node_id();
        sim_dev_api_send_broadcast_msg();

        sim_com_ping_network();
        sim_com_init_devices();
    }

    fn sim_dev_api_get_device_count() {
        let sim = DeviceSimAPI::new();
        sim.reset();

        assert_eq!(sim.get_device_count(), 0);
    }

    fn sim_dev_api_add_node() {
        let sim = DeviceSimAPI::new();
        sim.reset();

        assert_eq!(sim.get_device_count(), 0);
        assert_eq!(sim.add_device(1), true);
        assert_eq!(sim.get_device_count(), 1);
    }

    fn sim_dev_api_add_same_node_id() {
        let sim = DeviceSimAPI::new();
        sim.reset();

        assert_eq!(sim.get_device_count(), 0);
        assert_eq!(sim.add_device(1), true);
        assert_eq!(sim.get_device_count(), 1);
        assert_eq!(sim.add_device(1), false);
        assert_eq!(sim.get_device_count(), 1);
    }

    fn sim_dev_api_send_broadcast_msg() {
        let sim = DeviceSimAPI::new();
        sim.reset();

        sim.add_device(1);
        sim.add_device(2);
        assert_eq!(sim.get_device_count(), 2);

        let ping_msg = Msg::new_std_request(RequestType::Ping);
        sim.send_broadcast_msg(&ping_msg);
        sim.update_devices();

        assert!(sim.get_node_response_msg(1).is_none());
        assert!(sim.get_node_response_msg(2).is_none());

        for node_id in 0..2 {
            let (resp_node_id, response) = sim.get_broadcast_response_msg().unwrap();
            assert_eq!(resp_node_id, node_id + 1);
            assert!(ping_msg.is_response_ok(&response).is_ok());
        }

        assert!(sim.get_broadcast_response_msg().is_none());
    }

    fn sim_com_ping_network() {
        let sim_api = DeviceSimAPI::new();
        sim_api.reset();

        sim_api.add_device(1);
        sim_api.add_device(23);

        let node_id_lst = DeviceSimInterface::ping_network().unwrap();
        assert_eq!(node_id_lst.len(), 2);
        assert_eq!(node_id_lst[0], 1);
        assert_eq!(node_id_lst[1], 23);
    }

    fn sim_com_init_devices() {
        let sim_api = DeviceSimAPI::new();
        sim_api.reset();

        sim_api.add_device(1);
        sim_api.add_device(23);

        let node_id_lst = DeviceSimInterface::ping_network().unwrap();
        for node_id in node_id_lst {
            // Init devices
            let mut interface = DeviceSimInterface::open();
            interface.set_mode(ComMode::Specific(node_id)).unwrap();
            let mut device = Device::new(interface);

            assert!(device.init().is_ok());
            assert_eq!(device.get_bootloader_version(), "0.1.0");
            assert_eq!(device.get_device_info_vid(), 0x46524352);
            assert_eq!(device.get_device_info_pid(), 0x054455354);
            assert_eq!(device.get_device_info_uid(), 0);
        }
    }
}
