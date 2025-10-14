# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based CLI tool for updating firmware on embedded devices using the Frankly Bootloader protocol. The tool supports multiple communication interfaces (Serial, CAN, Ethernet, and SIM) and provides commands for searching, erasing, flashing, and resetting devices.

The project is organized as a Cargo workspace with three crates:
- **frankly-fw-update-common** (`common/`): Library containing the bootloader protocol implementation
- **frankly-fw-update-cli** (`cli/`): Binary crate providing the command-line interface
- **frankly-fw-update-tui** (`tui/`): Binary crate providing an interactive terminal UI interface using ratatui

## Build Commands

### Build the project
```bash
cargo build                              # Build entire workspace
cargo build -p frankly-fw-update-cli     # Build only the CLI
cargo build -p frankly-fw-update-tui     # Build only the TUI
cargo build -p frankly-fw-update-common  # Build only the library
```

### Run tests
```bash
cargo test                               # Run all tests in workspace
cargo test -p frankly-fw-update-common   # Run only library tests
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
cargo run -p frankly-fw-update-cli -- <command> --type <interface-type> --interface <interface-name> [--node <node-id>]
```

### Search for devices
```bash
cargo run -p frankly-fw-update-cli -- search --type can --interface can0
cargo run -p frankly-fw-update-cli -- search --type serial --interface ttyACM0
cargo run -p frankly-fw-update-cli -- search --type sim --interface sim
```

### Erase application
```bash
cargo run -p frankly-fw-update-cli -- erase --type can --interface can0 --node 1
```

### Flash firmware
```bash
cargo run -p frankly-fw-update-cli -- flash --type can --interface can0 --node 1 --hex-file path/to/firmware.hex
```

### Reset device
```bash
cargo run -p frankly-fw-update-cli -- reset --type can --interface can0 --node 1
```

## Running the TUI

The Terminal User Interface (TUI) provides an interactive menu-driven interface for all operations:

```bash
cargo run -p frankly-fw-update-tui
```

### TUI Features

The TUI provides an interactive, menu-driven experience with:
- **Main Menu**: Select operation (Search, Erase, Flash, Reset)
- **Interface Selection**: Choose interface type (SIM, Serial, CAN)
- **Input Forms**: Guided input for interface name, node ID, and hex file path
- **Operation Summary**: Review settings before execution
- **Results Display**: View operation results with color-coded success/error messages

### TUI Navigation
- **↑↓**: Navigate menu options
- **Enter**: Select/confirm
- **Esc**: Go back/cancel
- **q**: Quit application (when not in input mode)

### TUI Implementation
- Built with [ratatui](https://github.com/ratatui/ratatui) v0.29
- Uses crossterm v0.28 for terminal manipulation
- Shares core functionality with CLI via the common library

## Architecture

### High-Level Structure

The codebase is organized as a Cargo workspace with three crates:

1. **CLI Crate** (`cli/`): Command-line interface binary
   - `cli/src/main.rs`: CLI using `clap` that parses arguments and dispatches to appropriate operations

2. **TUI Crate** (`tui/`): Terminal user interface binary
   - `tui/src/main.rs`: Interactive TUI using `ratatui` with menu-driven operation selection

3. **Common Crate** (`common/`): Core bootloader protocol library
   - `common/src/francor/franklyboot/`: Protocol implementation
   - `common/build.rs`: Compiles C++ device simulator API from the parent `frankly-bootloader` repository
   - `common/tests/`: Integration tests with test data and utilities
   - `common/tests/utils/can_device_simulator/`: Python-based CAN device simulator for testing

### Communication Interface Abstraction

The `ComInterface` trait (`common/src/francor/franklyboot/com/mod.rs`) defines a unified interface for all communication methods:
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

The `Device<I: ComInterface>` struct (`common/src/francor/franklyboot/device/device.rs`) is generic over the communication interface, allowing the same device operations to work across all transport types.

The device manages:
- **Entry System**: Each bootloader command (RequestType) is wrapped as an Entry with a type (Const/RO/RW/Cmd)
- **Flash Description**: Memory layout with sections (Bootloader/Application)
- **Initialization**: Auto-reads constant device info (VID, PID, PRD, UID, flash layout) on `init()`

### Message Protocol

Messages are 8-byte structures (`common/src/francor/franklyboot/com/msg.rs`):
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

The `HexFile` struct (`common/src/francor/franklyboot/firmware/hex_file.rs`) implements Intel HEX format parsing and the `FirmwareDataInterface` trait to provide firmware data as a HashMap.

### C++ FFI Integration

The build script (`common/build.rs`) compiles C++ sources from the sibling `frankly-bootloader` repository:
- `francor/franklyboot/msg.cpp`: Message protocol implementation
- `device_sim_api.cpp`: Device simulator for testing

The Rust SIMInterface wraps these C++ functions to provide a simulated device for development and testing.

## Code Organization Patterns

### Error Handling
All bootloader operations return `Result<T, Error>` where `Error` is a custom enum (`common/src/francor/franklyboot/mod.rs`) with variants:
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
