# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based CLI tool for updating firmware on embedded devices using the Frankly Bootloader protocol. The tool supports multiple communication interfaces (Serial, CAN, Ethernet, and SIM) and provides commands for searching, erasing, flashing, and resetting devices.

## Build Commands

### Build the project
```bash
cargo build
```

### Run tests
```bash
cargo test              # Run all tests
cargo test --lib        # Run only library tests
```

### Build with dependencies
The project depends on the `frankly-bootloader` repository which must be cloned as a sibling directory:
```bash
cd ..
git clone https://github.com/franc0r/frankly-bootloader.git
cd frankly-bootloader && git checkout devel
cd ../frankly-fw-update-cli
cargo build
```

### System dependencies
On Linux systems, install:
```bash
sudo apt-get install -y libudev-dev
```

## Running the CLI

All commands follow this pattern:
```bash
cargo run -- <command> --type <interface-type> --interface <interface-name> [--node <node-id>]
```

### Search for devices
```bash
cargo run -- search --type can --interface can0
cargo run -- search --type serial --interface ttyACM0
cargo run -- search --type sim --interface sim
```

### Erase application
```bash
cargo run -- erase --type can --interface can0 --node 1
```

### Flash firmware
```bash
cargo run -- flash --type can --interface can0 --node 1 --hex-file path/to/firmware.hex
```

### Reset device
```bash
cargo run -- reset --type can --interface can0 --node 1
```

## Architecture

### High-Level Structure

The codebase is organized around a layered architecture:

1. **CLI Layer** (`src/main.rs`): Command-line interface using `clap` that parses arguments and dispatches to appropriate operations
2. **Library Layer** (`src/francor/franklyboot/`): Core bootloader protocol implementation
3. **Build Integration** (`build.rs`): Compiles C++ device simulator API from the parent `frankly-bootloader` repository

### Communication Interface Abstraction

The `ComInterface` trait (`src/francor/franklyboot/com/mod.rs`) defines a unified interface for all communication methods:
- **SerialInterface**: UART/USB serial communication
- **CANInterface**: CAN bus communication (supports multi-device networks)
- **SIMInterface**: Simulated device for testing (uses C++ FFI to device simulator)
- **ComSimulator**: Mock interface for unit testing

Key trait methods:
- `open()`: Establish connection with parameters
- `send()`/`recv()`: Message exchange
- `scan_network()`: Network discovery (for multi-device interfaces like CAN)
- `set_mode()`: Broadcast vs. specific node targeting

### Device Abstraction

The `Device<I: ComInterface>` struct (`src/francor/franklyboot/device/device.rs`) is generic over the communication interface, allowing the same device operations to work across all transport types.

The device manages:
- **Entry System**: Each bootloader command (RequestType) is wrapped as an Entry with a type (Const/RO/RW/Cmd)
- **Flash Description**: Memory layout with sections (Bootloader/Application)
- **Initialization**: Auto-reads constant device info (VID, PID, PRD, UID, flash layout) on `init()`

### Message Protocol

Messages are 8-byte structures (`src/francor/franklyboot/com/msg.rs`):
- Bytes 0-1: RequestType (command identifier)
- Byte 2: ResultType (status/error code)
- Byte 3: packet_id (for matching requests/responses)
- Bytes 4-7: MsgData (32-bit payload)

The protocol uses CRC validation and comprehensive error checking via `is_response_ok()`.

### Firmware Flashing Process

Flash operations (`Device::flash()`) follow this sequence:
1. Parse hex file into `FirmwareDataRaw` (HashMap of address -> byte)
2. Create `AppFirmware` representation organized into flash pages
3. For each page:
   - Clear device page buffer (`PageBufferClear`)
   - Write page data word-by-word (`PageBufferWriteWord`)
   - Verify page CRC (`PageBufferCalcCRC`)
   - Erase flash page (`FlashWriteErasePage`)
   - Write buffer to flash (`PageBufferWriteToFlash`)
4. Verify complete application CRC
5. Write application CRC to flash (`FlashWriteAppCRC`)
6. Start application (`StartApp`)

### HexFile Parsing

The `HexFile` struct (`src/francor/franklyboot/firmware/hex_file.rs`) implements Intel HEX format parsing and the `FirmwareDataInterface` trait to provide firmware data as a HashMap.

### C++ FFI Integration

The build script (`build.rs`) compiles C++ sources from the sibling `frankly-bootloader` repository:
- `francor/franklyboot/msg.cpp`: Message protocol implementation
- `device_sim_api.cpp`: Device simulator for testing

The Rust SIMInterface wraps these C++ functions to provide a simulated device for development and testing.

## Code Organization Patterns

### Error Handling
All bootloader operations return `Result<T, Error>` where `Error` is a custom enum (`src/francor/franklyboot/mod.rs`) with variants:
- `ComNoResponse`: Timeout waiting for device
- `ComError(String)`: Transport layer error
- `ResultError(String)`: Device returned error status
- `MsgCorruption(String)`: Protocol validation failure
- `NotSupported`: Feature not implemented
- `Error(String)`: General error

### Testing Strategy
- Unit tests are co-located with implementation (e.g., in `mod tests` blocks)
- `ComSimulator` provides a mock interface with injectable responses/errors
- SIMInterface enables integration testing without physical hardware

### Flash Memory Model
Flash is divided into sections:
- **Bootloader section**: Lower pages, read-only from CLI perspective
- **Application section**: Remaining pages, target for erase/flash operations

Pages are the smallest erasable unit. The `FlashDesc` struct maintains this layout and validates operations against section boundaries.

## Important Implementation Details

### Node ID Requirement
Multi-device network interfaces (CAN) require `--node` parameter for erase/flash/reset operations. The `search` command can discover available node IDs.

### CRC Algorithm
Uses CRC-32/ISO-HDLC (polynomial 0x04C11DB7) via the `crc` crate for:
- Per-page validation during transmission
- Complete application validation after flashing
- Bootloader integrity checking

### 128-bit UID
Device unique ID is read as four 32-bit words (UID1-4) and combined into a 128-bit value in `get_device_info_uid()`.

### Flash Default Value
Erased flash reads as 0xFF, defined as `FLASH_DFT_VALUE` constant. Unused bytes in firmware pages are filled with this value.
