use clap::{Arg, ArgAction, Command};
use frankly_fw_update_cli::francor::franklyboot::{
    com::{can::CANInterface, serial::SerialInterface, sim::SIMInterface, ComInterface, ComMode},
    device::Device,
    firmware::hex_file::HexFile,
};

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

pub fn search_for_sim_devices() {
    let node_lst = vec![1, 3, 31, 8];
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
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .help("Interface type \"sim\", \"serial\", \"can\"")
                        .required(true)
                        .action(ArgAction::Set)
                        .num_args(1),
                )
                .arg(
                    Arg::new("interface")
                        .short('i')
                        .long("interface")
                        .help("Interface name \"can0\", \"ttyACM0\", \"sim\"")
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
