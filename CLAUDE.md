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
- **Device Discovery**: Automatic scanning for available devices on selected interface
- **Device List Refresh**: Press F5 to rescan and update the device list
- **Input Forms**: Guided input for interface name, node ID, and hex file path
- **File Browser**: Interactive file system navigation to select hex files (only .hex files shown)
- **Operation Summary**: Review settings before execution
- **Results Display**: View operation results with color-coded success/error messages

### TUI Navigation
- **↑↓**: Navigate menu options
- **Enter**: Select/confirm
- **Tab**: Open file browser (when entering hex file path)
- **F5**: Refresh device list (rescan for devices on current interface)
- **Esc**: Go back/cancel
- **q**: Quit application (when not in input mode)

### File Browser
When selecting a hex file for flashing:
- Press **Tab** on the hex file input screen to open the interactive file browser
- Navigate directories using **↑↓** arrow keys
- Press **Enter** on a directory to open it (includes ".." to go to parent directory)
- Press **Enter** on a .hex file to select it and proceed with flashing
- Press **Esc** to return to manual path entry
- Only .hex files and directories are shown (hidden files are filtered out)
- Files are color-coded: directories in blue, .hex files in green

### TUI Implementation
- Built with [ratatui](https://github.com/ratatui/ratatui) v0.29
- Uses crossterm v0.28 for terminal manipulation
- **Live Progress Display**: Real-time progress bars during erase/flash operations
- **Background Threading**: Operations run in background threads to keep UI responsive
- **Channel-based Communication**: Progress updates flow from background threads via `mpsc::channel`
- Shares core functionality with CLI via the common library

## Architecture

### High-Level Structure

The codebase is organized as a Cargo workspace with three crates:

1. **CLI Crate** (`cli/`): Command-line interface binary
   - `cli/src/main.rs`: CLI using `clap` that parses arguments and dispatches to appropriate operations
   - Uses `indicatif` v0.17 for professional progress bars
   - Displays real-time progress during erase and flash operations

2. **TUI Crate** (`tui/`): Terminal user interface binary
   - `tui/src/main.rs`: Interactive TUI using `ratatui` with menu-driven operation selection
   - Background thread execution for non-blocking operations
   - Live progress bars with percentage and status updates
   - Event loop polls at 100ms intervals for smooth UI updates
   - **File Browser**: Interactive filesystem navigation for selecting hex files
     - Filters to show only .hex files and directories
     - Alphabetically sorted directories and files
     - Color-coded entries (blue for directories, green for files)
     - Accessible via Tab key from hex file input screen

3. **Common Crate** (`common/`): Core bootloader protocol library
   - `common/src/francor/franklyboot/`: Protocol implementation
   - **Output-agnostic design**: No direct stdout printing, uses callback architecture
   - `common/build.rs`: Compiles C++ device simulator API from the parent `frankly-bootloader` repository
   - `common/tests/`: Integration tests with test data and utilities
   - `common/tests/utils/can_device_simulator/`: Python-based CAN device simulator for testing

### Progress Reporting Architecture

**Key Design Principle**: The common library is completely output-agnostic and never writes to stdout. Applications control how progress is displayed via callbacks.

**ProgressUpdate Enum** (`common/src/francor/franklyboot/mod.rs`):
```rust
pub enum ProgressUpdate {
    Message(String),                          // General log messages
    EraseProgress { current: u32, total: u32 }, // Erase operation progress
    FlashProgress { current: u32, total: u32 }, // Flash operation progress
}
```

**Device Callback** (`common/src/francor/franklyboot/device/device.rs`):
- `Device::new_with_progress()` accepts optional progress callback
- Callback type: `Option<Box<dyn Fn(ProgressUpdate) + Send>>`
- All operations (reset, erase, flash) report progress via this callback
- Completely decouples protocol logic from presentation layer

**CLI Progress Handling** (`cli/src/main.rs`):
- Creates progress callback that updates `indicatif::ProgressBar`
- Initializes progress bar lazily on first progress update
- Shows: `[elapsed] [bar] current/total message`
- Example: `[00:05] [===================>----] 45/60 Erasing pages`

**TUI Progress Handling** (`tui/src/main.rs`):
- Spawns operations in background threads via `thread::spawn`
- Progress flows through `mpsc::channel` as `OperationMessage` enum
- Main thread polls channel with `try_recv()` every 100ms
- Updates UI state: `operation_progress`, `operation_status`
- Renders text-based progress bar: `[===================-----]`
- Automatically transitions to Results screen on completion

**Message Flow**:
```
Background Thread          Channel              UI Thread
──────────────────         ───────────          ─────────
device.erase()    ────>  EraseProgress(2/10) ──> Update display
device.erase()    ────>  EraseProgress(3/10) ──> Update display
...
device.erase()    ────>  Complete            ──> Show results
```

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
- **Progress Callback**: Optional `Box<dyn Fn(ProgressUpdate) + Send>` for reporting operation progress

**Device Constructors**:
- `Device::new(interface)` - Creates device without progress reporting
- `Device::new_with_progress(interface, progress_fn)` - Creates device with progress callback

**Progress Reporting**:
- `reset()` sends `Message("Reset device...")`
- `erase()` sends `EraseProgress { current, total }` for each page
- `flash()` sends `FlashProgress { current, total }` for each page
- Internal `progress()` helper safely invokes callback if present

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

## Progress Display Implementation

### CLI Progress Bars

The CLI uses the `indicatif` crate to show professional progress bars during long-running operations.

**Example Output**:
```
[00:00:05] [==================>-------------] 18/45 Erasing pages
```

**Implementation** (`cli/src/main.rs`):
```rust
let pb = Arc::new(Mutex::new(Option::<ProgressBar>::None));
let pb_clone = pb.clone();

let progress_fn = Some(Box::new(move |update: ProgressUpdate| {
    match update {
        ProgressUpdate::EraseProgress { current, total } => {
            let mut pb_lock = pb_clone.lock().unwrap();
            if pb_lock.is_none() {
                let bar = ProgressBar::new(total as u64);
                bar.set_style(/* template */);
                *pb_lock = Some(bar);
            }
            if let Some(ref bar) = *pb_lock {
                bar.set_position(current as u64);
            }
        }
        _ => {}
    }
}) as Box<dyn Fn(ProgressUpdate) + Send>);
```

**Key Features**:
- Lazy initialization: Progress bar created on first update
- Thread-safe: Uses `Arc<Mutex<>>` for shared state
- Elapsed time tracking
- Visual progress bar with customizable characters
- Completion message on finish

### TUI Live Progress

The TUI displays live progress without blocking the UI thread.

**Visual Display**:
```
╔═══════════════════════════════════════╗
║   Executing: Erase Application       ║
╠═══════════════════════════════════════╣
║ Device: Node   1 - VID: 0x46524352   ║
║ Command: Erase Application            ║
║                                       ║
║ Progress: [==================------]  ║
║           18/45 pages (40%)           ║
║                                       ║
║ Erasing page 18/45                    ║
╚═══════════════════════════════════════╝
```

**Implementation** (`tui/src/main.rs`):

**OperationMessage Enum**:
```rust
enum OperationMessage {
    Progress(ProgressUpdate),  // Progress update from device
    DeviceInfo(String),        // Device identification
    Complete,                  // Operation finished successfully
    Error(String),             // Operation failed
}
```

**Background Thread Pattern**:
```rust
fn spawn_erase<I: ComInterface + 'static>(&self, tx: Sender<OperationMessage>, ...) {
    thread::spawn(move || {
        let progress_tx = tx.clone();
        let progress_fn = Some(Box::new(move |update: ProgressUpdate| {
            progress_tx.send(OperationMessage::Progress(update)).ok();
        }) as Box<dyn Fn(ProgressUpdate) + Send>);

        // ... connect to device with progress_fn ...

        match device.erase() {
            Ok(_) => tx.send(OperationMessage::Complete).ok(),
            Err(e) => tx.send(OperationMessage::Error(format!("{:?}", e))).ok(),
        }
    });
}
```

**Message Processing Loop**:
```rust
fn process_operation_messages(&mut self) {
    if let Some(ref receiver) = self.operation_receiver {
        while let Ok(msg) = receiver.try_recv() {
            match msg {
                OperationMessage::Progress(ProgressUpdate::EraseProgress { current, total }) => {
                    self.operation_progress = Some((current, total));
                    self.operation_status = format!("Erasing page {}/{}", current, total);
                }
                OperationMessage::Complete => {
                    self.current_screen = Screen::Results;
                }
                // ... other messages ...
            }
        }
    }
}
```

**Event Loop Integration**:
```rust
fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        app.process_operation_messages();  // Poll for progress updates
        terminal.draw(|f| ui(f, app))?;

        // Poll for user input with timeout to keep UI responsive
        if crossterm::event::poll(Duration::from_millis(100))? {
            // Handle keyboard input...
        }
    }
}
```

**Key Features**:
- Non-blocking execution: UI remains responsive during operations
- 100ms polling interval: Smooth progress updates
- Text-based progress bar: `[=====>-----]` rendered in TUI
- Percentage calculation: `(current / total * 100)%`
- Status messages: "Erasing page X/Y" or "Flashing page X/Y"
- Automatic screen transition: Moves to Results when complete
- Error handling: Displays errors and returns to Results screen

### Benefits of Callback Architecture

1. **Separation of Concerns**: Protocol logic separate from UI presentation
2. **Testability**: Common library can be tested without UI dependencies
3. **Flexibility**: Easy to add new UIs (GUI, web interface) without changing core
4. **No stdout Pollution**: TUI never has unwanted output breaking display
5. **Consistent Progress**: Both CLI and TUI get same detailed progress information
6. **Thread Safety**: Callbacks marked `Send` for cross-thread communication
