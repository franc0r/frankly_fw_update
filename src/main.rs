//use clap::{Arg, ArgAction, Command};
use frankly_fw_update_cli::francor::franklyboot::{
    com::{can::CANInterface, serial::SerialInterface, sim::SIMInterface},
    device::Device,
    firmware::hex_file::HexFile,
};

pub enum InterfaceType {
    Serial,
    CAN,
    Ethernet,
}

pub fn search_for_devices(interface_type: InterfaceType, interface_name: &String) {
    match interface_type {
        InterfaceType::Serial => {
            println!("Searching for devices on serial port {}", interface_name);
        }
        InterfaceType::CAN => {
            println!("Searching for devices on CAN bus {}", interface_name);
        }
        InterfaceType::Ethernet => {
            println!("Searching for devices on Ethernet {}", interface_name);
        }
    }
}

pub fn run_can_test() {
    let node_lst = CANInterface::ping_network("can0").unwrap();

    println!("Found nodes: {:?}", node_lst);

    let mut device = Device::new(CANInterface::open("can0").unwrap());
    device.init().unwrap();
    device.erase().unwrap();
}

pub fn run_serial_test() {
    let mut device = Device::new(SerialInterface::open("/dev/ttyACM0", 115200).unwrap());
    device.init().unwrap();
    device.erase().unwrap();

    device
        .flash(&HexFile::from_file("./tests/data/example_app_g431rb.hex").unwrap())
        .unwrap();
}

fn main() {
    // run_can_test();
    run_serial_test();

    // TODO: Implement sim device network to support com trait!
    // Run tests on sim network
}

// Tests ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sim_net_ping() {
        let node_lst = vec![1, 3, 31, 8];
        SIMInterface::config_nodes(node_lst).unwrap();
    }
}
