use frankly_fw_update_cli::francor::franklyboot::{
    com::{can::CANInterface, serial::SerialInterface},
    device::Device,
    firmware::hex_file::HexFile,
    Error,
};

//#[test]
//fn device_flash() {
//    let firmware = HexFile::from_file("./tests/data/TestFirmware.hex").unwrap();
//    let mut device = Device::new();
//    let mut com = new_com_sim_with_data();
//
//    device.init(&mut com).unwrap();
//
//    device.flash(&mut com, &firmware).unwrap();
//}

fn main() {
    // Create new device
    let mut device = Device::new();

    // Create new serial interface
    //let mut com = SerialInterface::open("/dev/ttyACM0", 115200).unwrap();
    let mut com = CANInterface::open("can0").unwrap();

    device.init(&mut com).unwrap();
    println!("Device: {}", device);

    // Open firmware file
    let firmware = HexFile::from_file("./tests/data/example_app_g431rb.hex").unwrap();
    //let firmware = HexFile::from_file("./tests/data/TestG431RBBlinky.hex").unwrap();

    // Flash firmware
    match device.flash(&mut com, &firmware) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
        }
    }
}
