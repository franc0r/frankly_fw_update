// C++ API ----------------------------------------------------------------------------------------

//
// Device SIM C++ API
//
#[link(name = "franklyboot-device-sim-api", kind = "static")]
extern "C" {
    fn SIM_reset();
    fn SIM_addDevice(node_id: u8) -> bool;
    fn SIM_getDeviceCount() -> u32;
    fn SIM_sendBroadcastMsg(msg_ptr: *const u8);
    fn SIM_sendNodeMsg(node_id: u8, raw_msg_ptr: *const u8);
    fn SIM_updateDevices();
    fn SIM_getBroadcastResponseMsg(node_id: *mut u8, raw_msg_ptr: *mut u8) -> bool;
    fn SIM_getNodeResponseMsg(node_id: u8, raw_msg_ptr: *mut u8) -> bool;
}

// Public Defines ---------------------------------------------------------------------------------

pub const MSG_SIZE: usize = 8;

// Public Functions -------------------------------------------------------------------------------

///
/// Reset the device simulator
///
pub fn reset() {
    unsafe {
        SIM_reset();
    }
}

///
/// Add device
///
pub fn add_device(node_id: u8) -> Result<(), String> {
    let result = unsafe { SIM_addDevice(node_id) };

    if result {
        Ok(())
    } else {
        Err("Failed to add device".to_string())
    }
}

///
/// Get device count
///
pub fn get_device_count() -> u32 {
    unsafe { SIM_getDeviceCount() }
}

///
/// Send broadcast message
///
pub fn send_broadcast_msg(msg: &[u8; MSG_SIZE]) {
    let msg_ptr = msg as *const u8;

    unsafe {
        SIM_sendBroadcastMsg(msg_ptr);
    }

    update_devices();
}

///
/// Send node message
///
pub fn send_node_msg(node_id: u8, msg: &[u8; MSG_SIZE]) {
    let msg_ptr = msg as *const u8;

    unsafe {
        SIM_sendNodeMsg(node_id, msg_ptr);
    }

    update_devices();
}

///
/// Update devices
///
fn update_devices() {
    unsafe {
        SIM_updateDevices();
    }
}

///
/// Get broadcast response message
///
pub fn get_broadcast_response_msg() -> Option<(u8, [u8; MSG_SIZE])> {
    let mut node_id: u8 = 0;
    let mut msg: [u8; MSG_SIZE] = [0; MSG_SIZE];
    let node_id_ptr = &mut node_id as *mut u8;
    let msg_ptr = &mut msg as *mut u8;

    let result = unsafe { SIM_getBroadcastResponseMsg(node_id_ptr, msg_ptr) };

    if result {
        Some((node_id, msg))
    } else {
        None
    }
}

///
/// Get node response message
///
pub fn get_node_response_msg(node_id: u8) -> Option<[u8; MSG_SIZE]> {
    let mut msg: [u8; MSG_SIZE] = [0; MSG_SIZE];
    let msg_ptr = &mut msg as *mut u8;

    let result = unsafe { SIM_getNodeResponseMsg(node_id, msg_ptr) };

    if result {
        Some(msg)
    } else {
        None
    }
}

// Tests ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_device() {
        reset();

        assert_eq!(get_device_count(), 0);

        add_device(1).unwrap();

        assert_eq!(get_device_count(), 1);
    }

    #[test]
    fn test_send_broadcast_msg() {
        reset();

        add_device(1).unwrap();
        add_device(2).unwrap();

        let msg: [u8; MSG_SIZE] = [1, 0, 0, 0, 0, 0, 0, 0];
        let exp_resp: [u8; MSG_SIZE] = [1, 0, 1, 0, 0, 1, 0, 0];

        send_broadcast_msg(&msg);

        let response = get_broadcast_response_msg();

        assert!(response.is_some());

        let (node_id, response_msg) = response.unwrap();

        assert_eq!(node_id, 1);
        assert_eq!(response_msg, exp_resp);

        let response = get_broadcast_response_msg();

        assert!(response.is_some());

        let (node_id, response_msg) = response.unwrap();

        assert_eq!(node_id, 2);
        assert_eq!(response_msg, exp_resp);
    }

    #[test]
    fn test_send_node_msg() {
        reset();

        add_device(1).unwrap();
        add_device(2).unwrap();

        let msg: [u8; MSG_SIZE] = [1, 0, 0, 0, 0, 0, 0, 0];
        let exp_resp: [u8; MSG_SIZE] = [1, 0, 1, 0, 0, 1, 0, 0];

        send_node_msg(1, &msg);

        let response = get_node_response_msg(1);

        assert!(response.is_some());

        let response_msg = response.unwrap();

        assert_eq!(response_msg, exp_resp);
    }
}
