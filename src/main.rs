use clap::{Arg, ArgAction, Command};
use frankly_fw_update_cli::francor::franklyboot::{
    com::{can::CANInterface, serial::SerialInterface, sim::SIMInterface, ComInterface, ComMode},
    device::Device,
    firmware::hex_file::HexFile,
};

const SIM_NODE_LST: [u8; 4] = [1, 3, 31, 8];

pub enum InterfaceType {
    Sim,
    Serial,
    CAN,
    Ethernet,
}

pub fn search_for_devices(interface_type: InterfaceType, interface_name: &String) {
    match interface_type {
        InterfaceType::Serial => {
            println!(
                "--> Searching for devices on serial port {}",
                interface_name
            );
            search_for_serial_devices(interface_name);
        }
        InterfaceType::CAN => {
            println!("--> Searching for devices on CAN bus {}", interface_name);
        }
        InterfaceType::Ethernet => {
            println!("--> Searching for devices on Ethernet {}", interface_name);
        }
        InterfaceType::Sim => {
            println!("--> Searching for devices on simulated network");
            search_for_sim_devices();
        }
    }
}

pub fn erase_device(interface_type: InterfaceType, interface_name: &String, node: u8) {
    match interface_type {
        InterfaceType::Serial => {
            println!(
                "--> Erasing device on serial bus {} with node id {}",
                interface_name, node
            );

            let interface = SerialInterface::open(interface_name, 115200).unwrap();
            let mut device = Device::new(interface);
            device.init().unwrap();
            device.erase().unwrap();
        }
        InterfaceType::CAN => {
            println!(
                "--> Erasing devices on CAN bus {} with node id {}",
                interface_name, node
            );
        }
        InterfaceType::Ethernet => {
            println!(
                "--> Erasing devices on Ethernet {} with node id {}",
                interface_name, node
            );
        }
        InterfaceType::Sim => {
            println!(
                "--> Erasing devices on simulated network with node id {}",
                node
            );

            let node_lst = SIM_NODE_LST.to_vec();
            SIMInterface::config_nodes(node_lst).unwrap();
            let mut interface = SIMInterface::open("").unwrap();
            interface.set_mode(ComMode::Specific(node)).unwrap();
            let mut device = Device::new(interface);
            device.init().unwrap();
            device.erase().unwrap();
        }
    }
}

pub fn search_for_sim_devices() {
    let node_lst = SIM_NODE_LST.to_vec();
    SIMInterface::config_nodes(node_lst).unwrap();
    let node_lst = SIMInterface::ping_network().unwrap();

    for node in node_lst {
        let mut interface = SIMInterface::open("").unwrap();
        interface.set_mode(ComMode::Specific(node)).unwrap();
        let mut device = Device::new(interface);
        device.init().unwrap();

        println!("Device found[{:3}]: {}", node, device);
    }
}

pub fn search_for_serial_devices(interface_name: &String) {
    let interface = SerialInterface::open(interface_name, 115200).unwrap();
    let mut device = Device::new(interface);
    device.init().unwrap();

    println!("Device found: {}", device);
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
    let type_arg = Arg::new("type")
        .short('t')
        .long("type")
        .help("Interface type \"sim\", \"serial\", \"can\"")
        .required(true)
        .action(ArgAction::Set)
        .num_args(1);

    let interface_arg = Arg::new("interface")
        .short('i')
        .long("interface")
        .help("Interface name \"can0\", \"ttyACM0\", \"sim\"")
        .required(true)
        .action(ArgAction::Set)
        .num_args(1);

    let node_arg = Arg::new("node")
        .short('n')
        .long("node")
        .help("Node ID: 0, 1, ..")
        .value_parser(clap::value_parser!(u8).range(0..))
        .required(true)
        .action(ArgAction::Set)
        .num_args(1);

    let matches = Command::new("frankly-fw-update")
        .version("0.1.0")
        .author("Martin Bauernschmitt - FRANCOR e.V.")
        .arg_required_else_help(true)
        .subcommand_required(true)
        .subcommand(
            Command::new("search")
                .short_flag('s')
                .long_flag("search")
                .about("Search for connected devices on specified network")
                .arg(type_arg.clone())
                .arg(interface_arg.clone()),
        )
        .subcommand(
            Command::new("erase")
                .short_flag('e')
                .long_flag("erase")
                .about("Erases the application from the device")
                .arg(type_arg.clone())
                .arg(interface_arg.clone())
                .arg(node_arg.clone()),
        )
        .subcommand(
            Command::new("flash")
                .short_flag('f')
                .long_flag("flash")
                .about("Flashes the application to the device")
                .arg(type_arg.clone())
                .arg(interface_arg.clone())
                .arg(node_arg.clone())
                .arg(
                    Arg::new("hex-file")
                        .long("hex-file")
                        .help("Path to hex file")
                        .required(true)
                        .action(ArgAction::Set)
                        .num_args(1),
                ),
        )
        .get_matches();

    println!("Frankly Firmware Update CLI (c) 2021 Martin Bauernschmitt - FRANCOR e.V.");

    match matches.subcommand() {
        Some(("search", search_matches)) => {
            let interface_name = search_matches.get_one::<String>("interface").unwrap();
            let interface_type = search_matches.get_one::<String>("type").unwrap();

            if interface_type == "serial" {
                search_for_devices(InterfaceType::Serial, &interface_name);
            } else if interface_type == "can" {
                search_for_devices(InterfaceType::CAN, &interface_name);
            } else if interface_type == "sim" {
                search_for_devices(InterfaceType::Sim, &interface_name);
            } else {
                println!("Unknown interface type {}", interface_type);
            }
        }
        Some(("erase", erase_matches)) => {
            let interface_name = erase_matches.get_one::<String>("interface").unwrap();
            let interface_type = erase_matches.get_one::<String>("type").unwrap();
            let node_id = *erase_matches.get_one::<u8>("node").unwrap();

            if interface_type == "serial" {
                erase_device(InterfaceType::Serial, &interface_name, node_id);
            } else if interface_type == "can" {
                erase_device(InterfaceType::CAN, &interface_name, node_id);
            } else if interface_type == "sim" {
                erase_device(InterfaceType::Sim, &interface_name, node_id);
            } else {
                println!("Unknown interface type {}", interface_type);
            }
        }
        Some(("flash", flash_matches)) => {
            let interface_name = flash_matches.get_one::<String>("interface").unwrap();
            let interface_type = flash_matches.get_one::<String>("type").unwrap();
            let node_id = *flash_matches.get_one::<u8>("node").unwrap();
            let hex_file_path = flash_matches.get_one::<String>("hex-file").unwrap();

            if interface_type == "serial" {
                println!(
                    "--> Flashing {} device on serial bus {} with node id {}",
                    hex_file_path, interface_name, node_id
                );

                // Open interface
                let interface = SerialInterface::open(interface_name, 115200).unwrap();

                // Open hex file
                let hex_file = HexFile::from_file(hex_file_path).unwrap();

                // Create device
                let mut device = Device::new(interface);

                // Init device
                device.init().unwrap();

                // Flash device
                device.flash(&hex_file).unwrap();
            } else if interface_type == "can" {
                println!(
                    "--> Flashing {} devices on CAN bus {} with node id {}",
                    hex_file_path, interface_name, node_id
                );
            } else if interface_type == "sim" {
                // Create sim network
                let node_lst = SIM_NODE_LST.to_vec();
                SIMInterface::config_nodes(node_lst).unwrap();

                // Open interface
                let mut interface = SIMInterface::open("").unwrap();
                interface.set_mode(ComMode::Specific(node_id)).unwrap();

                // Open hex file
                let hex_file = HexFile::from_file(hex_file_path).unwrap();

                // Create device
                let mut device = Device::new(interface);

                // Init device
                device.init().unwrap();

                // Erase device
                device.erase().unwrap();

                // Flash device
                device.flash(&hex_file).unwrap();

                println!(
                    "--> Flashing {} devices on simulated network with node id {}",
                    hex_file_path, node_id
                );
            } else {
                println!("Unknown interface type {}", interface_type);
            }
        }
        _ => {
            println!("Unknown command");
        }
    }
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
