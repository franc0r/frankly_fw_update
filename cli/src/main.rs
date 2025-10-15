use clap::{Arg, ArgAction, Command};
use frankly_fw_update_common::francor::franklyboot::{
    com::{
        can::CANInterface, serial::SerialInterface, sim::SIMInterface, ComConnParams, ComInterface,
        ComMode,
    },
    device::Device,
    firmware::hex_file::HexFile,
    Error, ProgressUpdate,
};
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::{Arc, Mutex};

const SIM_NODE_LST: [u8; 4] = [1, 3, 31, 8];

pub enum InterfaceType {
    Sim,
    Serial,
    CAN,
    Ethernet,
}

// Convert from string to interface type
impl InterfaceType {
    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "sim" => Ok(InterfaceType::Sim),
            "serial" => Ok(InterfaceType::Serial),
            "can" => Ok(InterfaceType::CAN),
            "ethernet" => Ok(InterfaceType::Ethernet),
            _ => Err(Error::Error(format!("Unknown interface type {}", s))),
        }
    }
}

fn main() {
    create_sim_devices();

    let type_arg = Arg::new("type")
        .long("type")
        .help("Interface type \"sim\", \"serial\", \"can\"")
        .required(true)
        .action(ArgAction::Set)
        .num_args(1);

    let interface_arg = Arg::new("interface")
        .long("interface")
        .help("Interface name \"can0\", \"ttyACM0\", \"sim\"")
        .required(true)
        .action(ArgAction::Set)
        .num_args(1);

    let node_arg = Arg::new("node")
        .long("node")
        .help("Node ID: 1, 2, ..")
        .value_parser(clap::value_parser!(u8).range(0..))
        .required(false)
        .action(ArgAction::Set)
        .num_args(1);

    let matches = Command::new("frankly-fw-update")
        .version("0.1.0")
        .author("Martin Bauernschmitt - FRANCOR e.V.")
        .arg_required_else_help(true)
        .subcommand_required(true)
        .subcommand(
            Command::new("search")
                .long_flag("search")
                .about("Search for connected devices on specified network")
                .arg(type_arg.clone())
                .arg(interface_arg.clone()),
        )
        .subcommand(
            Command::new("erase")
                .long_flag("erase")
                .about("Erases the application from the device")
                .arg(type_arg.clone())
                .arg(interface_arg.clone())
                .arg(node_arg.clone()),
        )
        .subcommand(
            Command::new("flash")
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
        .subcommand(
            Command::new("reset")
                .long_flag("reset")
                .about("Reset device")
                .arg(type_arg.clone())
                .arg(interface_arg.clone())
                .arg(node_arg.clone()),
        )
        .get_matches();

    println!("Frankly Firmware Update CLI (c) 2023 Martin Bauernschmitt - FRANCOR e.V.");

    match matches.subcommand() {
        Some(("search", search_matches)) => {
            let interface_type_str = search_matches.get_one::<String>("type").unwrap();
            let interface_type = InterfaceType::from_str(&interface_type_str).unwrap();
            let interface_name = search_matches.get_one::<String>("interface").unwrap();

            match interface_type {
                InterfaceType::Serial => search_for_devices::<SerialInterface>(
                    &ComConnParams::for_serial_conn(interface_name, 115200),
                ),
                InterfaceType::CAN => {
                    search_for_devices::<CANInterface>(&ComConnParams::for_can_conn(interface_name))
                }
                InterfaceType::Ethernet => {
                    println!("Ethernet not supported yet");
                }
                InterfaceType::Sim => {
                    search_for_devices::<SIMInterface>(&ComConnParams::for_sim_device())
                }
            }
        }
        Some(("erase", erase_matches)) => {
            let interface_type_str = erase_matches.get_one::<String>("type").unwrap();
            let interface_type = InterfaceType::from_str(interface_type_str).unwrap();
            let interface_name = erase_matches.get_one::<String>("interface").unwrap();
            let node_id = match erase_matches.get_one::<u8>("node") {
                Some(v) => Some(*v),
                None => None,
            };

            match interface_type {
                InterfaceType::Serial => erase_device::<SerialInterface>(
                    &ComConnParams::for_serial_conn(interface_name, 115200),
                    node_id,
                ),
                InterfaceType::CAN => erase_device::<CANInterface>(
                    &ComConnParams::for_can_conn(interface_name),
                    node_id,
                ),
                InterfaceType::Ethernet => println!("Ethernet not supported yet"),
                InterfaceType::Sim => {
                    erase_device::<SIMInterface>(&ComConnParams::for_sim_device(), node_id)
                }
            }
        }
        Some(("flash", flash_matches)) => {
            let interface_type_str = flash_matches.get_one::<String>("type").unwrap();
            let interface_type = InterfaceType::from_str(interface_type_str).unwrap();
            let interface_name = flash_matches.get_one::<String>("interface").unwrap();
            let node_id = match flash_matches.get_one::<u8>("node") {
                Some(v) => Some(*v),
                None => None,
            };
            let hex_file_path = flash_matches.get_one::<String>("hex-file").unwrap();

            match interface_type {
                InterfaceType::Serial => flash_device::<SerialInterface>(
                    &ComConnParams::for_serial_conn(interface_name, 115200),
                    node_id,
                    &hex_file_path,
                ),
                InterfaceType::CAN => flash_device::<CANInterface>(
                    &ComConnParams::for_can_conn(interface_name),
                    node_id,
                    &hex_file_path,
                ),
                InterfaceType::Ethernet => println!("Ethernet not supported yet"),
                InterfaceType::Sim => flash_device::<SIMInterface>(
                    &ComConnParams::for_sim_device(),
                    node_id,
                    &hex_file_path,
                ),
            }
        }
        Some(("reset", reset_matches)) => {
            let interface_type_str = reset_matches.get_one::<String>("type").unwrap();
            let interface_type = InterfaceType::from_str(&interface_type_str).unwrap();
            let interface_name = reset_matches.get_one::<String>("interface").unwrap();
            let node_id = match reset_matches.get_one::<u8>("node") {
                Some(v) => Some(*v),
                None => None,
            };

            match interface_type {
                InterfaceType::Serial => reset_device::<SerialInterface>(
                    &ComConnParams::for_serial_conn(interface_name, 115200),
                    node_id,
                ),
                InterfaceType::CAN => reset_device::<CANInterface>(
                    &ComConnParams::for_can_conn(interface_name),
                    node_id,
                ),
                InterfaceType::Ethernet => println!("Ethernet not supported yet"),
                InterfaceType::Sim => {
                    reset_device::<SIMInterface>(&ComConnParams::for_sim_device(), node_id)
                }
            }
        }
        _ => {
            println!("Unknown command");
        }
    }
}

pub fn connect_device<I>(
    conn_params: &ComConnParams,
    node_id: Option<u8>,
    progress_fn: Option<Box<dyn Fn(ProgressUpdate) + Send>>,
) -> Result<Device<I>, Error>
where
    I: ComInterface,
{
    let mut interface = I::create()?;
    interface.open(conn_params)?;
    if node_id.is_some() {
        interface.set_mode(ComMode::Specific(node_id.unwrap()))?;
    }

    let mut device = Device::new_with_progress(interface, progress_fn);
    device.init()?;

    Ok(device)
}

pub fn search_for_devices<I>(conn_params: &ComConnParams)
where
    I: ComInterface,
{
    if I::is_network() {
        let node_lst = {
            let mut interface = I::create().unwrap();
            interface.open(conn_params).unwrap();
            interface.scan_network().unwrap()
        };

        for node in node_lst {
            let device = connect_device::<I>(conn_params, Some(node), None).unwrap();
            println!("Device found[{:3}]: {}", node, device);
        }
    } else {
        let device = connect_device::<I>(conn_params, None, None).unwrap();
        println!("Device found: {}", device);
    }
}

pub fn reset_device<I>(conn_params: &ComConnParams, node_id: Option<u8>)
where
    I: ComInterface,
{
    if I::is_network() {
        if node_id.is_none() {
            println!("Node ID required for multi device network interface! Specify with --node <node-id>");
            return;
        }
    }

    let progress_fn = Some(Box::new(|update: ProgressUpdate| match update {
        ProgressUpdate::Message(msg) => println!("{}", msg),
        _ => {}
    }) as Box<dyn Fn(ProgressUpdate) + Send>);

    let mut device = connect_device::<I>(conn_params, node_id, progress_fn).unwrap();
    println!("Device: {}", device);
    device.reset().unwrap();
}

pub fn erase_device<I>(conn_params: &ComConnParams, node_id: Option<u8>)
where
    I: ComInterface,
{
    if I::is_network() {
        if node_id.is_none() {
            println!("Node ID required for multi device network interface! Specify with --node <node-id>");
            return;
        }
    }

    let pb = Arc::new(Mutex::new(Option::<ProgressBar>::None));
    let pb_clone = pb.clone();

    let progress_fn = Some(Box::new(move |update: ProgressUpdate| match update {
        ProgressUpdate::EraseProgress { current, total } => {
            let mut pb_lock = pb_clone.lock().unwrap();
            if pb_lock.is_none() {
                let bar = ProgressBar::new(total as u64);
                bar.set_style(
                    ProgressStyle::default_bar()
                        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                bar.set_message("Erasing pages");
                *pb_lock = Some(bar);
            }
            if let Some(ref bar) = *pb_lock {
                bar.set_position(current as u64);
                if current == total {
                    bar.finish_with_message("Erase complete");
                }
            }
        }
        ProgressUpdate::Message(msg) => println!("{}", msg),
        _ => {}
    }) as Box<dyn Fn(ProgressUpdate) + Send>);

    let mut device = connect_device::<I>(conn_params, node_id, progress_fn).unwrap();
    println!("Device: {}", device);
    device.erase().unwrap();
}

pub fn flash_device<I>(conn_params: &ComConnParams, node_id: Option<u8>, hex_file_path: &str)
where
    I: ComInterface,
{
    if I::is_network() {
        if node_id.is_none() {
            println!("Node ID required for multi device network interface! Specify with --node <node-id>");
            return;
        }
    }

    let pb = Arc::new(Mutex::new(Option::<ProgressBar>::None));
    let pb_clone = pb.clone();

    let progress_fn = Some(Box::new(move |update: ProgressUpdate| match update {
        ProgressUpdate::FlashProgress { current, total } => {
            let mut pb_lock = pb_clone.lock().unwrap();
            if pb_lock.is_none() {
                let bar = ProgressBar::new(total as u64);
                bar.set_style(
                    ProgressStyle::default_bar()
                        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                bar.set_message("Flashing pages");
                *pb_lock = Some(bar);
            }
            if let Some(ref bar) = *pb_lock {
                bar.set_position(current as u64);
                if current == total {
                    bar.finish_with_message("Flash complete");
                }
            }
        }
        ProgressUpdate::Message(msg) => println!("{}", msg),
        _ => {}
    }) as Box<dyn Fn(ProgressUpdate) + Send>);

    let mut device = connect_device::<I>(conn_params, node_id, progress_fn).unwrap();
    println!("Device: {}", device);

    let hex_file = HexFile::from_file(hex_file_path).unwrap();
    device.flash(&hex_file).unwrap();
}

fn create_sim_devices() {
    let node_lst = SIM_NODE_LST.to_vec();
    SIMInterface::config_nodes(node_lst).unwrap();
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
