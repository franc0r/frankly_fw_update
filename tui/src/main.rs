//! # Frankly Firmware Update - Terminal User Interface (TUI)
//!
//! Interactive terminal-based user interface for updating firmware on embedded devices
//! using the Frankly Bootloader protocol.
//!
//! ## Architecture Overview
//!
//! The TUI follows a **screen-based state machine** architecture with background task execution:
//!
//! ```text
//! InterfaceTypeSelection → InterfaceSelection → Searching → DeviceList
//!                                                              ↓
//!                                                         CommandMenu
//!                                                              ↓
//!                                                       HexFileInput ←→ FileBrowser
//!                                                              ↓
//!                                                          Executing
//!                                                              ↓
//!                                                           Results
//! ```
//!
//! ## Key Features
//!
//! - **Non-blocking Operations**: Long-running operations (search, erase, flash) execute in
//!   background threads, keeping the UI responsive
//! - **Live Progress Updates**: Real-time progress bars for erase/flash operations via
//!   message-passing channels
//! - **Device Discovery**: Automatic scanning for devices on selected interface, with F5 refresh
//! - **File Browser**: Interactive filesystem navigation for selecting hex files
//! - **History Management**: Remembers last 10 firmware file paths for quick reuse
//! - **Multi-Interface Support**: Works with Serial, CAN, and SIM (simulated) interfaces
//!
//! ## Message Passing Architecture
//!
//! Background operations communicate with the UI thread via `mpsc::channel`:
//!
//! ```text
//! Background Thread           Channel              UI Thread
//! ─────────────────          ─────────            ─────────
//! device.erase()      ──>  EraseProgress(2/10) ──> Update progress bar
//! device.flash()      ──>  FlashProgress(5/60) ──> Update status message
//! operation complete  ──>  Complete            ──> Transition to Results screen
//! ```
//!
//! The UI thread polls channels every 100ms using `try_recv()` to maintain responsiveness.
//!
//! ## Screen Flow Details
//!
//! 1. **InterfaceTypeSelection**: Choose between SIM, Serial, or CAN
//! 2. **InterfaceSelection**: Select specific interface (e.g., /dev/ttyACM0, can0)
//! 3. **Searching**: Background device discovery with progress overlay
//! 4. **DeviceList**: Display found devices, select target device
//! 5. **CommandMenu**: Choose operation (Reset, Erase, Flash)
//! 6. **HexFileInput**: Enter firmware path (with history) or press Tab for browser
//! 7. **FileBrowser**: Navigate filesystem to select .hex file
//! 8. **Executing**: Live progress display during operation execution
//! 9. **Results**: Show operation outcome (success/error)
//!
//! ## Threading Model
//!
//! - **Main Thread**: UI rendering and event handling (60 FPS with 100ms poll)
//! - **Background Threads**: Device operations (spawned via `thread::spawn`)
//! - **Communication**: Unidirectional via `mpsc::channel` (background → main)

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use frankly_fw_update_common::francor::franklyboot::{
    com::{
        can::CANInterface, serial::SerialInterface, sim::SIMInterface, ComConnParams, ComInterface,
        ComMode,
    },
    device::Device,
    firmware::hex_file::HexFile,
    Error, ProgressUpdate,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

/// Default list of simulated device node IDs for testing
const SIM_NODE_LST: [u8; 4] = [1, 3, 31, 8];

// ================================================================================================
// Type Definitions
// ================================================================================================

/// Communication interface type selection.
///
/// Represents the available communication protocols for connecting to embedded devices.
/// Each type has different characteristics:
///
/// - **Sim**: Simulated devices for testing without hardware
/// - **Serial**: UART/USB serial connections (single device per port)
/// - **CAN**: CAN bus networks (supports multiple devices on single bus)
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum InterfaceType {
    /// Simulated device interface for testing
    Sim,
    /// Serial (UART/USB) interface
    Serial,
    /// CAN bus interface
    CAN,
}

impl InterfaceType {
    /// Returns the human-readable display name for this interface type
    fn as_str(&self) -> &str {
        match self {
            InterfaceType::Sim => "SIM",
            InterfaceType::Serial => "Serial",
            InterfaceType::CAN => "CAN",
        }
    }
}

/// Represents a discovered device on the network or interface.
///
/// Contains identification information retrieved during the device search phase.
/// For network interfaces (CAN), includes the node ID. For point-to-point interfaces
/// (Serial), node_id is None.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DiscoveredDevice {
    /// Node ID for network interfaces (CAN), None for point-to-point (Serial)
    node_id: Option<u8>,
    /// Human-readable display name shown in device list
    display_name: String,
    /// Full device information string (VID, PID, PRD, UID)
    device_info: String,
}

/// Application screen states representing the UI state machine.
///
/// Each variant corresponds to a distinct screen in the TUI. Navigation between
/// screens follows the flow defined in the module-level documentation.
#[derive(Debug, Clone, PartialEq)]
enum Screen {
    /// Initial screen: Select interface type (SIM/Serial/CAN)
    InterfaceTypeSelection,
    /// Select specific interface instance (e.g., ttyACM0, can0)
    InterfaceSelection,
    /// Searching for devices (shows progress overlay)
    Searching,
    /// Display list of discovered devices
    DeviceList,
    /// Select command to execute (Reset/Erase/Flash)
    CommandMenu,
    /// Input hex file path with history support
    HexFileInput,
    /// Interactive file browser for selecting hex files
    FileBrowser,
    /// Executing operation with live progress display
    Executing,
    /// Show operation results (success/error)
    Results,
}

/// Bootloader commands that can be executed on a device.
#[derive(Debug, Clone, PartialEq)]
enum Command {
    /// Reset the device (restart application or stay in bootloader)
    Reset,
    /// Erase the application flash memory
    Erase,
    /// Flash new firmware from hex file
    Flash,
}

impl Command {
    /// Returns the human-readable display name for this command
    fn as_str(&self) -> &str {
        match self {
            Command::Reset => "Reset Device",
            Command::Erase => "Erase Application",
            Command::Flash => "Flash Firmware",
        }
    }
}

/// Messages sent from background operation threads to the UI thread.
///
/// These messages flow through an `mpsc::channel` to provide progress updates
/// and completion status for long-running operations (reset, erase, flash).
#[derive(Debug)]
enum OperationMessage {
    /// Progress update from device operation (erase/flash progress, status messages)
    Progress(ProgressUpdate),
    /// Device identification information retrieved after connection
    DeviceInfo(String),
    /// Operation completed successfully
    Complete,
    /// Operation failed with error message
    Error(String),
}

/// Messages sent from background search threads to the UI thread.
///
/// Device discovery runs in a background thread to avoid blocking the UI.
/// Results are streamed back via these messages.
#[derive(Debug)]
enum SearchMessage {
    /// A device was discovered on the interface
    DeviceFound(DiscoveredDevice),
    /// Search completed (all devices found)
    Complete,
    /// Search failed with error message
    Error(String),
}

/// Represents a file or directory entry in the file browser.
///
/// Used for interactive filesystem navigation when selecting hex files.
#[derive(Debug, Clone)]
struct FileEntry {
    /// File or directory name (not full path)
    name: String,
    /// Full absolute path to the entry
    path: PathBuf,
    /// True if this is a directory, false if it's a file
    is_dir: bool,
}

// ================================================================================================
// Application State
// ================================================================================================

/// Main application state container.
///
/// Holds all UI state, user selections, discovered devices, and communication channels
/// for background operations. This is the central data structure that drives the entire TUI.
///
/// ## State Management
///
/// - **Screen Navigation**: `current_screen` tracks which UI screen is displayed
/// - **User Selections**: Interface type, specific interface, device, command, hex file
/// - **Discovery State**: List of discovered devices and available interfaces
/// - **Background Tasks**: Receivers for operation and search messages
/// - **UI State**: List selections, input modes, history, error/result messages
struct App {
    // === Screen and Navigation ===
    /// Current active screen in the state machine
    current_screen: Screen,

    // === Interface Type Selection ===
    /// List widget state for interface type selection
    interface_type_state: ListState,
    /// Currently selected interface type (SIM/Serial/CAN)
    selected_interface_type: Option<InterfaceType>,

    // === Interface Selection ===
    /// Available interface instances (e.g., ["/dev/ttyACM0", "/dev/ttyUSB0"])
    available_interfaces: Vec<String>,
    /// List widget state for interface selection
    interface_list_state: ListState,
    /// Currently selected interface instance
    selected_interface: Option<String>,

    // === Device Discovery ===
    /// List of devices found during search operation
    discovered_devices: Vec<DiscoveredDevice>,
    /// List widget state for device list
    device_list_state: ListState,
    /// Index of selected device in discovered_devices
    selected_device_index: Option<usize>,

    // === Command Selection ===
    /// List widget state for command menu
    command_menu_state: ListState,
    /// Currently selected command (Reset/Erase/Flash)
    selected_command: Option<Command>,

    // === Hex File Input ===
    /// Current hex file path being entered or selected
    hex_file_path: String,
    /// Whether hex file input is in text entry mode (vs browsing)
    hex_file_input_mode: bool,
    /// History of previously used firmware file paths (max 10)
    hex_file_history: Vec<String>,
    /// Current position in history when browsing with arrow keys
    hex_file_history_index: Option<usize>,

    // === Results and Messages ===
    /// Success messages to display on Results screen
    result_message: Vec<String>,
    /// Error message to display (if any)
    error_message: Option<String>,
    /// Temporary message shown after refreshing device list
    device_list_refresh_message: Option<String>,

    // === Progress Tracking ===
    /// Current operation progress: (current_page, total_pages)
    operation_progress: Option<(u32, u32)>,
    /// Status message for current operation (e.g., "Erasing page 5/10")
    operation_status: String,
    /// Channel receiver for operation progress updates from background thread
    operation_receiver: Option<Receiver<OperationMessage>>,

    // === Search Tracking ===
    /// Channel receiver for device search results from background thread
    search_receiver: Option<Receiver<SearchMessage>>,
    /// Flag indicating whether current search is a refresh operation
    is_refresh_search: bool,

    // === File Browser ===
    /// Current directory being browsed in file browser
    file_browser_current_dir: PathBuf,
    /// List of files and directories in current directory
    file_browser_entries: Vec<FileEntry>,
    /// List widget state for file browser
    file_browser_list_state: ListState,
}

// ================================================================================================
// Application Implementation
// ================================================================================================

impl App {
    /// Creates a new App instance with default state.
    ///
    /// Initializes all UI state, sets the starting screen to InterfaceTypeSelection,
    /// and prepares list selections with appropriate defaults.
    fn new() -> App {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut app = App {
            current_screen: Screen::InterfaceTypeSelection,
            interface_type_state: ListState::default(),
            selected_interface_type: None,
            available_interfaces: Vec::new(),
            interface_list_state: ListState::default(),
            selected_interface: None,
            discovered_devices: Vec::new(),
            device_list_state: ListState::default(),
            selected_device_index: None,
            command_menu_state: ListState::default(),
            selected_command: None,
            hex_file_path: String::new(),
            hex_file_input_mode: false,
            hex_file_history: Vec::new(),
            hex_file_history_index: None,
            result_message: Vec::new(),
            error_message: None,
            device_list_refresh_message: None,
            operation_progress: None,
            operation_status: String::new(),
            operation_receiver: None,
            search_receiver: None,
            is_refresh_search: false,
            file_browser_current_dir: current_dir,
            file_browser_entries: Vec::new(),
            file_browser_list_state: ListState::default(),
        };
        // Select first item in interface type list by default
        app.interface_type_state.select(Some(0));
        // Select first item in command menu by default
        app.command_menu_state.select(Some(0));
        app
    }

    /// Discovers and enumerates available interfaces for the selected interface type.
    ///
    /// This method populates `available_interfaces` with a list of interface instances
    /// that can be used for communication. The behavior varies by interface type:
    ///
    /// - **SIM**: Always returns a single "sim" interface
    /// - **Serial**: Scans for accessible serial ports, filtering out inactive/inaccessible ones
    /// - **CAN**: Scans `/sys/class/net` for CAN interfaces (can*, vcan*)
    ///
    /// ## Serial Port Filtering
    ///
    /// For Serial interfaces, each port is tested by attempting to open it with a 100ms timeout.
    /// Only ports that can be successfully opened are included in the list. This filters out:
    /// - Ports that are already in use
    /// - Ports without proper permissions
    /// - Inactive/disconnected ports
    ///
    /// ## Error Handling
    ///
    /// Sets `error_message` if no interfaces are found or enumeration fails.
    fn discover_interfaces(&mut self) {
        self.available_interfaces.clear();

        match self.selected_interface_type.as_ref().unwrap() {
            InterfaceType::Sim => {
                self.available_interfaces.push("sim".to_string());
            }
            InterfaceType::Serial => {
                // Enumerate serial ports and filter to only accessible ones
                match serialport::available_ports() {
                    Ok(ports) => {
                        for port in ports {
                            // Try to open the port to verify it's accessible and active
                            // Use a very short timeout to avoid hanging
                            match serialport::new(&port.port_name, 115200)
                                .timeout(Duration::from_millis(100))
                                .open()
                            {
                                Ok(_) => {
                                    // Port is accessible and active, add it to the list
                                    self.available_interfaces.push(port.port_name);
                                }
                                Err(_) => {
                                    // Port is not accessible or inactive, skip it
                                }
                            }
                        }
                    }
                    Err(_) => {
                        self.error_message = Some("Failed to enumerate serial ports".to_string());
                    }
                }

                if self.available_interfaces.is_empty() {
                    self.error_message = Some("No accessible serial ports found".to_string());
                }
            }
            InterfaceType::CAN => {
                // Enumerate CAN interfaces from /sys/class/net
                if let Ok(entries) = fs::read_dir("/sys/class/net") {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        // Check if it's a CAN interface (can* or vcan*)
                        if name.starts_with("can") || name.starts_with("vcan") {
                            self.available_interfaces.push(name);
                        }
                    }
                }

                if self.available_interfaces.is_empty() {
                    self.error_message = Some("No CAN interfaces found".to_string());
                }
            }
        }

        // Select first interface if any were found
        if !self.available_interfaces.is_empty() {
            self.interface_list_state.select(Some(0));
        }
    }

    /// Adds a firmware file path to the history list.
    ///
    /// Maintains a history of the last 10 firmware file paths used for flashing.
    /// The history is ordered with most recent first, and duplicates are removed.
    ///
    /// ## Behavior
    ///
    /// - Empty paths are ignored
    /// - If path already exists in history, it's moved to the front
    /// - History is limited to 10 entries (oldest are dropped)
    /// - History index is reset after adding
    ///
    /// Users can navigate history with ↑↓ arrow keys in the HexFileInput screen.
    fn add_to_hex_file_history(&mut self, path: String) {
        // Don't add empty paths or duplicates at the front
        if path.is_empty() {
            return;
        }

        // Remove the path if it already exists
        self.hex_file_history.retain(|p| p != &path);

        // Add to the front of history
        self.hex_file_history.insert(0, path);

        // Limit history to 10 entries
        if self.hex_file_history.len() > 10 {
            self.hex_file_history.truncate(10);
        }

        // Reset history index when adding new item
        self.hex_file_history_index = None;
    }

    /// Executes the currently selected command on the selected device.
    ///
    /// This method orchestrates command execution by:
    /// 1. Preparing execution parameters (connection params, node ID, hex file)
    /// 2. Creating a progress update channel
    /// 3. Spawning a background thread for the operation
    /// 4. Delegating to interface-specific spawn methods
    ///
    /// ## Background Execution
    ///
    /// Operations run in background threads via `spawn_operation`, `spawn_erase`, or
    /// `spawn_flash` methods. Progress updates flow back to the UI thread through
    /// `operation_receiver`.
    ///
    /// ## Command Types
    ///
    /// - **Reset**: Spawns via `spawn_operation`
    /// - **Erase**: Spawns via `spawn_erase` with page-by-page progress
    /// - **Flash**: Spawns via `spawn_flash`, includes hex file loading and verification
    fn execute_command(&mut self) {
        self.result_message.clear();
        self.error_message = None;
        self.operation_progress = None;
        self.operation_status = String::new();

        let command = match &self.selected_command {
            Some(cmd) => cmd.clone(),
            None => return,
        };

        let interface_type = match &self.selected_interface_type {
            Some(it) => it.clone(),
            None => return,
        };

        // Add hex file path to history if this is a flash command
        if matches!(command, Command::Flash) && !self.hex_file_path.is_empty() {
            self.add_to_hex_file_history(self.hex_file_path.clone());
        }

        // Create channel for progress updates
        let (tx, rx) = channel();
        self.operation_receiver = Some(rx);

        // Get execution parameters
        let conn_params = self.get_conn_params();
        let device_node = self.get_selected_device().and_then(|d| d.node_id);
        let hex_file_path = self.hex_file_path.clone();

        // Spawn background thread for operation
        match interface_type {
            InterfaceType::Sim => {
                SIMInterface::config_nodes(SIM_NODE_LST.to_vec()).ok();
                match command {
                    Command::Reset => {
                        self.spawn_operation::<SIMInterface>(tx, conn_params, device_node, None)
                    }
                    Command::Erase => {
                        self.spawn_erase::<SIMInterface>(tx, conn_params, device_node)
                    }
                    Command::Flash => self.spawn_flash::<SIMInterface>(
                        tx,
                        conn_params,
                        device_node,
                        hex_file_path,
                    ),
                }
            }
            InterfaceType::Serial => match command {
                Command::Reset => {
                    self.spawn_operation::<SerialInterface>(tx, conn_params, device_node, None)
                }
                Command::Erase => self.spawn_erase::<SerialInterface>(tx, conn_params, device_node),
                Command::Flash => {
                    self.spawn_flash::<SerialInterface>(tx, conn_params, device_node, hex_file_path)
                }
            },
            InterfaceType::CAN => match command {
                Command::Reset => {
                    self.spawn_operation::<CANInterface>(tx, conn_params, device_node, None)
                }
                Command::Erase => self.spawn_erase::<CANInterface>(tx, conn_params, device_node),
                Command::Flash => {
                    self.spawn_flash::<CANInterface>(tx, conn_params, device_node, hex_file_path)
                }
            },
        }
    }

    fn spawn_operation<I: ComInterface + 'static>(
        &self,
        tx: Sender<OperationMessage>,
        conn_params: ComConnParams,
        node_id: Option<u8>,
        _hex_file: Option<String>,
    ) {
        thread::spawn(move || {
            // Create progress callback
            let progress_tx = tx.clone();
            let progress_fn = Some(Box::new(move |update: ProgressUpdate| {
                progress_tx.send(OperationMessage::Progress(update)).ok();
            }) as Box<dyn Fn(ProgressUpdate) + Send>);

            // Connect to device
            let mut interface = match I::create() {
                Ok(i) => i,
                Err(e) => {
                    tx.send(OperationMessage::Error(format!(
                        "Failed to create interface: {:?}",
                        e
                    )))
                    .ok();
                    return;
                }
            };

            if let Err(e) = interface.open(&conn_params) {
                tx.send(OperationMessage::Error(format!(
                    "Failed to open interface: {:?}",
                    e
                )))
                .ok();
                return;
            }

            if let Some(node) = node_id {
                if let Err(e) = interface.set_mode(ComMode::Specific(node)) {
                    tx.send(OperationMessage::Error(format!(
                        "Failed to set node mode: {:?}",
                        e
                    )))
                    .ok();
                    return;
                }
            }

            let mut device = Device::new_with_progress(interface, progress_fn);
            if let Err(e) = device.init() {
                tx.send(OperationMessage::Error(format!(
                    "Failed to initialize device: {:?}",
                    e
                )))
                .ok();
                return;
            }

            // Send device info
            let device_info = format!("{}", device)
                .replace('\t', " ")
                .replace('\r', "")
                .replace('\n', " ");
            tx.send(OperationMessage::DeviceInfo(device_info)).ok();

            // Execute reset
            match device.reset() {
                Ok(_) => {
                    tx.send(OperationMessage::Complete).ok();
                }
                Err(e) => {
                    tx.send(OperationMessage::Error(format!("Reset failed: {:?}", e)))
                        .ok();
                }
            }
        });
    }

    fn spawn_erase<I: ComInterface + 'static>(
        &self,
        tx: Sender<OperationMessage>,
        conn_params: ComConnParams,
        node_id: Option<u8>,
    ) {
        thread::spawn(move || {
            let progress_tx = tx.clone();
            let progress_fn = Some(Box::new(move |update: ProgressUpdate| {
                progress_tx.send(OperationMessage::Progress(update)).ok();
            }) as Box<dyn Fn(ProgressUpdate) + Send>);

            let mut interface = match I::create() {
                Ok(i) => i,
                Err(e) => {
                    tx.send(OperationMessage::Error(format!(
                        "Failed to create interface: {:?}",
                        e
                    )))
                    .ok();
                    return;
                }
            };

            if let Err(e) = interface.open(&conn_params) {
                tx.send(OperationMessage::Error(format!(
                    "Failed to open interface: {:?}",
                    e
                )))
                .ok();
                return;
            }

            if let Some(node) = node_id {
                if let Err(e) = interface.set_mode(ComMode::Specific(node)) {
                    tx.send(OperationMessage::Error(format!(
                        "Failed to set node mode: {:?}",
                        e
                    )))
                    .ok();
                    return;
                }
            }

            let mut device = Device::new_with_progress(interface, progress_fn);
            if let Err(e) = device.init() {
                tx.send(OperationMessage::Error(format!(
                    "Failed to initialize device: {:?}",
                    e
                )))
                .ok();
                return;
            }

            let device_info = format!("{}", device)
                .replace('\t', " ")
                .replace('\r', "")
                .replace('\n', " ");
            tx.send(OperationMessage::DeviceInfo(device_info)).ok();

            match device.erase() {
                Ok(_) => {
                    tx.send(OperationMessage::Complete).ok();
                }
                Err(e) => {
                    tx.send(OperationMessage::Error(format!("Erase failed: {:?}", e)))
                        .ok();
                }
            }
        });
    }

    fn spawn_flash<I: ComInterface + 'static>(
        &self,
        tx: Sender<OperationMessage>,
        conn_params: ComConnParams,
        node_id: Option<u8>,
        hex_file_path: String,
    ) {
        thread::spawn(move || {
            let hex_file = match HexFile::from_file(&hex_file_path) {
                Ok(hf) => hf,
                Err(e) => {
                    tx.send(OperationMessage::Error(format!(
                        "Failed to load hex file: {:?}",
                        e
                    )))
                    .ok();
                    return;
                }
            };

            let progress_tx = tx.clone();
            let progress_fn = Some(Box::new(move |update: ProgressUpdate| {
                progress_tx.send(OperationMessage::Progress(update)).ok();
            }) as Box<dyn Fn(ProgressUpdate) + Send>);

            let mut interface = match I::create() {
                Ok(i) => i,
                Err(e) => {
                    tx.send(OperationMessage::Error(format!(
                        "Failed to create interface: {:?}",
                        e
                    )))
                    .ok();
                    return;
                }
            };

            if let Err(e) = interface.open(&conn_params) {
                tx.send(OperationMessage::Error(format!(
                    "Failed to open interface: {:?}",
                    e
                )))
                .ok();
                return;
            }

            if let Some(node) = node_id {
                if let Err(e) = interface.set_mode(ComMode::Specific(node)) {
                    tx.send(OperationMessage::Error(format!(
                        "Failed to set node mode: {:?}",
                        e
                    )))
                    .ok();
                    return;
                }
            }

            let mut device = Device::new_with_progress(interface, progress_fn);
            if let Err(e) = device.init() {
                tx.send(OperationMessage::Error(format!(
                    "Failed to initialize device: {:?}",
                    e
                )))
                .ok();
                return;
            }

            let device_info = format!("{}", device)
                .replace('\t', " ")
                .replace('\r', "")
                .replace('\n', " ");
            tx.send(OperationMessage::DeviceInfo(device_info)).ok();

            match device.flash(&hex_file) {
                Ok(_) => {
                    tx.send(OperationMessage::Complete).ok();
                }
                Err(e) => {
                    tx.send(OperationMessage::Error(format!("Flash failed: {:?}", e)))
                        .ok();
                }
            }
        });
    }

    fn get_selected_device(&self) -> Option<&DiscoveredDevice> {
        self.selected_device_index
            .and_then(|idx| self.discovered_devices.get(idx))
    }

    /// Returns the connection parameters for the currently selected interface.
    ///
    /// Creates a `ComConnParams` struct appropriate for the selected interface type:
    /// - **SIM**: Simulated device parameters
    /// - **Serial**: Serial port name + 115200 baud rate
    /// - **CAN**: CAN interface name
    fn get_conn_params(&self) -> ComConnParams {
        let interface_name = self.selected_interface.as_ref().unwrap();
        match self.selected_interface_type.as_ref().unwrap() {
            InterfaceType::Sim => ComConnParams::for_sim_device(),
            InterfaceType::Serial => ComConnParams::for_serial_conn(interface_name, 115200),
            InterfaceType::CAN => ComConnParams::for_can_conn(interface_name),
        }
    }

    /// Processes progress messages from background operation threads.
    ///
    /// Called every 100ms from the main event loop to check for updates from
    /// `operation_receiver`. Updates UI state based on received messages:
    ///
    /// - `Progress`: Updates progress bar and status message
    /// - `DeviceInfo`: Adds device identification to results
    /// - `Complete`: Marks operation successful and transitions to Results screen
    /// - `Error`: Captures error message and transitions to Results screen
    ///
    /// ## Non-blocking Design
    ///
    /// Uses `try_recv()` to avoid blocking the UI thread. Processes all pending
    /// messages in a tight loop before returning control to the event handler.
    fn process_operation_messages(&mut self) {
        let mut operation_complete = false;
        let mut operation_error = None;

        if let Some(ref receiver) = self.operation_receiver {
            // Non-blocking check for messages
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    OperationMessage::Progress(update) => match update {
                        ProgressUpdate::EraseProgress { current, total } => {
                            self.operation_progress = Some((current, total));
                            self.operation_status = format!("Erasing page {}/{}", current, total);
                        }
                        ProgressUpdate::FlashProgress { current, total } => {
                            self.operation_progress = Some((current, total));
                            self.operation_status = format!("Flashing page {}/{}", current, total);
                        }
                        ProgressUpdate::Message(msg) => {
                            self.operation_status = msg;
                        }
                    },
                    OperationMessage::DeviceInfo(info) => {
                        self.result_message.push(format!("Device: {}", info));
                    }
                    OperationMessage::Complete => {
                        self.result_message
                            .push("Operation completed successfully".to_string());
                        operation_complete = true;
                    }
                    OperationMessage::Error(err) => {
                        operation_error = Some(err);
                    }
                }
            }
        }

        // Handle completion after the borrow ends
        if operation_complete || operation_error.is_some() {
            self.operation_receiver = None;
            self.current_screen = Screen::Results;
            if let Some(err) = operation_error {
                self.error_message = Some(err);
            }
        }
    }

    /// Populates the file browser with entries from the current directory.
    ///
    /// Scans `file_browser_current_dir` and builds a list of navigable entries:
    /// - Adds parent directory (`..`) if not at filesystem root
    /// - Includes all subdirectories (for navigation)
    /// - Includes only `.hex` files (filters out other file types)
    /// - Skips hidden files (starting with `.`)
    /// - Sorts entries: directories first (alphabetically), then files (alphabetically)
    ///
    /// ## Display Format
    ///
    /// - Directories: `[DIR] dirname/` (colored blue)
    /// - Hex files: `[FILE] filename.hex` (colored green)
    fn populate_file_browser(&mut self) {
        self.file_browser_entries.clear();

        // Add parent directory entry if not at root
        if self.file_browser_current_dir.parent().is_some() {
            self.file_browser_entries.push(FileEntry {
                name: "..".to_string(),
                path: self
                    .file_browser_current_dir
                    .parent()
                    .unwrap()
                    .to_path_buf(),
                is_dir: true,
            });
        }

        // Read directory entries
        if let Ok(entries) = fs::read_dir(&self.file_browser_current_dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();

            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Skip hidden files (starting with .)
                    if name.starts_with('.') {
                        continue;
                    }

                    if metadata.is_dir() {
                        dirs.push(FileEntry {
                            name,
                            path,
                            is_dir: true,
                        });
                    } else if metadata.is_file() {
                        // Only show .hex files
                        if path.extension().and_then(|s| s.to_str()) == Some("hex") {
                            files.push(FileEntry {
                                name,
                                path,
                                is_dir: false,
                            });
                        }
                    }
                }
            }

            // Sort directories and files alphabetically
            dirs.sort_by(|a, b| a.name.cmp(&b.name));
            files.sort_by(|a, b| a.name.cmp(&b.name));

            // Add directories first, then files
            self.file_browser_entries.extend(dirs);
            self.file_browser_entries.extend(files);
        }

        // Select first entry
        if !self.file_browser_entries.is_empty() {
            self.file_browser_list_state.select(Some(0));
        } else {
            self.file_browser_list_state.select(None);
        }
    }

    fn enter_file_browser(&mut self) {
        // Start from current working directory
        self.file_browser_current_dir =
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        self.populate_file_browser();
    }

    /// Initiates an asynchronous device search operation.
    ///
    /// Spawns a background thread that scans the selected interface for devices.
    /// Results are streamed back via `search_receiver` and processed by
    /// `process_search_messages()`.
    ///
    /// ## Search Behavior by Interface Type
    ///
    /// - **Network Interfaces (CAN, SIM)**: Calls `scan_network()` to get node list,
    ///   then connects to each node individually to retrieve device info
    /// - **Point-to-Point (Serial)**: Attempts single device connection on the port
    ///
    /// ## Message Flow
    ///
    /// ```text
    /// Background Thread          Channel           UI Thread
    /// ─────────────────         ────────          ─────────
    /// scan_network()      ──>  DeviceFound(1)  ──> Add to list
    /// connect_to_node(1)  ──>  DeviceFound(2)  ──> Add to list
    /// ...                 ──>  Complete        ──> Show device list
    /// ```
    fn spawn_search(&mut self) {
        self.discovered_devices.clear();
        self.error_message = None;

        let interface_type = match &self.selected_interface_type {
            Some(it) => it.clone(),
            None => return,
        };

        let interface_name = match &self.selected_interface {
            Some(name) => name.clone(),
            None => return,
        };

        // Create channel for search updates
        let (tx, rx) = channel();
        self.search_receiver = Some(rx);

        // Spawn background thread for search
        thread::spawn(move || {
            let conn_params = match interface_type {
                InterfaceType::Sim => ComConnParams::for_sim_device(),
                InterfaceType::Serial => ComConnParams::for_serial_conn(&interface_name, 115200),
                InterfaceType::CAN => ComConnParams::for_can_conn(&interface_name),
            };

            match interface_type {
                InterfaceType::Sim => {
                    SIMInterface::config_nodes(SIM_NODE_LST.to_vec()).ok();
                    Self::search_devices_async::<SIMInterface>(tx, conn_params);
                }
                InterfaceType::Serial => {
                    Self::search_devices_async::<SerialInterface>(tx, conn_params);
                }
                InterfaceType::CAN => {
                    Self::search_devices_async::<CANInterface>(tx, conn_params);
                }
            }
        });
    }

    fn search_devices_async<I: ComInterface + 'static>(
        tx: Sender<SearchMessage>,
        conn_params: ComConnParams,
    ) {
        if I::is_network() {
            // Multi-device network interface (CAN, SIM)
            match I::create() {
                Ok(mut interface) => {
                    if let Err(e) = interface.open(&conn_params) {
                        tx.send(SearchMessage::Error(format!(
                            "Failed to open interface: {:?}",
                            e
                        )))
                        .ok();
                        return;
                    }
                    match interface.scan_network() {
                        Ok(node_lst) => {
                            for node in node_lst {
                                match Self::connect_and_get_info::<I>(&conn_params, Some(node)) {
                                    Ok((device_info, display_name)) => {
                                        tx.send(SearchMessage::DeviceFound(DiscoveredDevice {
                                            node_id: Some(node),
                                            display_name,
                                            device_info,
                                        }))
                                        .ok();
                                    }
                                    Err(e) => {
                                        tx.send(SearchMessage::DeviceFound(DiscoveredDevice {
                                            node_id: Some(node),
                                            display_name: format!(
                                                "Node {:3} - Error: {:?}",
                                                node, e
                                            ),
                                            device_info: String::new(),
                                        }))
                                        .ok();
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tx.send(SearchMessage::Error(format!(
                                "Network scan failed: {:?}",
                                e
                            )))
                            .ok();
                            return;
                        }
                    }
                }
                Err(e) => {
                    tx.send(SearchMessage::Error(format!(
                        "Failed to create interface: {:?}",
                        e
                    )))
                    .ok();
                    return;
                }
            }
        } else {
            // Single device interface (Serial)
            match Self::connect_and_get_info::<I>(&conn_params, None) {
                Ok((device_info, display_name)) => {
                    tx.send(SearchMessage::DeviceFound(DiscoveredDevice {
                        node_id: None,
                        display_name,
                        device_info,
                    }))
                    .ok();
                }
                Err(e) => {
                    tx.send(SearchMessage::Error(format!("Failed to connect: {:?}", e)))
                        .ok();
                    return;
                }
            }
        }

        tx.send(SearchMessage::Complete).ok();
    }

    fn connect_and_get_info<I: ComInterface>(
        conn_params: &ComConnParams,
        node_id: Option<u8>,
    ) -> Result<(String, String), Error> {
        let mut interface = I::create()?;
        interface.open(conn_params)?;
        if let Some(node) = node_id {
            interface.set_mode(ComMode::Specific(node))?;
        }

        let mut device = Device::new(interface);
        device.init()?;

        let device_info = format!("{}", device)
            .replace('\t', " ")
            .replace('\r', "")
            .replace('\n', " ");

        let display_name = if let Some(node) = node_id {
            format!("Node {:3} - {}", node, device_info)
        } else {
            device_info.clone()
        };

        Ok((device_info, display_name))
    }

    fn process_search_messages(&mut self) {
        let mut search_complete = false;
        let mut search_error = None;

        if let Some(ref receiver) = self.search_receiver {
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    SearchMessage::DeviceFound(device) => {
                        self.discovered_devices.push(device);
                    }
                    SearchMessage::Complete => {
                        search_complete = true;
                    }
                    SearchMessage::Error(err) => {
                        search_error = Some(err);
                    }
                }
            }
        }

        // Handle completion after the borrow ends
        if search_complete || search_error.is_some() {
            self.search_receiver = None;

            if let Some(err) = search_error {
                self.error_message = Some(err);
            }

            if self.discovered_devices.is_empty() && self.error_message.is_none() {
                self.error_message = Some("No devices found".to_string());
            }

            // Set refresh message if this was a refresh operation
            if self.is_refresh_search {
                let device_count = self.discovered_devices.len();
                self.device_list_refresh_message = Some(format!(
                    "Device list refreshed - Found {} device(s)",
                    device_count
                ));
                self.is_refresh_search = false;
            }

            if !self.discovered_devices.is_empty() {
                self.device_list_state.select(Some(0));
                self.current_screen = Screen::DeviceList;
            } else if self.error_message.is_some() {
                self.current_screen = Screen::Results;
            } else {
                self.current_screen = Screen::DeviceList;
            }
        }
    }
}

// ================================================================================================
// Main Entry Point
// ================================================================================================

/// Application entry point.
///
/// Sets up the terminal in raw mode with alternate screen, runs the TUI event loop,
/// and restores terminal state on exit.
///
/// ## Terminal Setup
///
/// - Enables raw mode (disables line buffering, echo)
/// - Switches to alternate screen (preserves main terminal content)
/// - Enables mouse capture for potential future enhancements
///
/// ## Error Handling
///
/// Ensures terminal is properly restored even if the app panics or returns an error.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal for TUI mode
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state and run event loop
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal to original state
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Print any errors that occurred during execution
    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

/// Main event loop for the TUI application.
///
/// Continuously:
/// 1. Processes messages from background threads (operations and searches)
/// 2. Redraws the UI based on current app state
/// 3. Polls for keyboard input with 100ms timeout
/// 4. Dispatches input to appropriate screen handler
///
/// ## Performance
///
/// - Polls at 100ms intervals (10 FPS) for responsive UI
/// - Non-blocking message processing via `try_recv()`
/// - Only redraws when state changes or input occurs
///
/// ## Input Handling
///
/// Each screen has its own keyboard handler function. The global 'q' key
/// quits the application (except when in text input mode).
fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        // Poll background threads for progress updates
        app.process_operation_messages();
        app.process_search_messages();

        // Render current screen
        terminal.draw(|f| ui(f, app))?;

        // Poll for keyboard input with 100ms timeout (keeps UI responsive)
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Dispatch to screen-specific handler
                    match app.current_screen {
                        Screen::InterfaceTypeSelection => {
                            handle_interface_type_selection(app, key.code)
                        }
                        Screen::InterfaceSelection => handle_interface_selection(app, key.code),
                        Screen::Searching => {} // Blocked during search
                        Screen::DeviceList => handle_device_list(app, key.code),
                        Screen::CommandMenu => handle_command_menu(app, key.code),
                        Screen::HexFileInput => handle_hex_file_input(app, key.code),
                        Screen::FileBrowser => handle_file_browser(app, key.code),
                        Screen::Executing => {} // Blocked during execution
                        Screen::Results => handle_results(app, key.code),
                    }

                    // Global quit shortcut (unless in text input mode)
                    if let KeyCode::Char('q') = key.code {
                        if !app.hex_file_input_mode {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

// ================================================================================================
// Keyboard Input Handlers
// ================================================================================================
//
// Each screen has a dedicated handler function that processes KeyCode events.
// Handlers update app state and trigger screen transitions as needed.
//
// Common patterns:
// - Up/Down: Navigate lists
// - Enter: Select/confirm
// - Esc: Go back to previous screen
// - F5: Refresh lists
// ================================================================================================

fn handle_interface_type_selection(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down => {
            let i = match app.interface_type_state.selected() {
                Some(i) => {
                    if i >= 2 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            app.interface_type_state.select(Some(i));
        }
        KeyCode::Up => {
            let i = match app.interface_type_state.selected() {
                Some(i) => {
                    if i == 0 {
                        2
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            app.interface_type_state.select(Some(i));
        }
        KeyCode::Enter => {
            let selected = app.interface_type_state.selected().unwrap_or(0);
            app.selected_interface_type = Some(match selected {
                0 => InterfaceType::Sim,
                1 => InterfaceType::Serial,
                2 => InterfaceType::CAN,
                _ => InterfaceType::Sim,
            });
            app.discover_interfaces();
            if !app.available_interfaces.is_empty() {
                app.current_screen = Screen::InterfaceSelection;
            }
        }
        _ => {}
    }
}

fn handle_interface_selection(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down => {
            let max_idx = app.available_interfaces.len().saturating_sub(1);
            let i = match app.interface_list_state.selected() {
                Some(i) => {
                    if i >= max_idx {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            app.interface_list_state.select(Some(i));
        }
        KeyCode::Up => {
            let max_idx = app.available_interfaces.len().saturating_sub(1);
            let i = match app.interface_list_state.selected() {
                Some(i) => {
                    if i == 0 {
                        max_idx
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            app.interface_list_state.select(Some(i));
        }
        KeyCode::Enter => {
            if let Some(idx) = app.interface_list_state.selected() {
                if let Some(interface) = app.available_interfaces.get(idx) {
                    app.selected_interface = Some(interface.clone());
                    app.current_screen = Screen::Searching;
                    app.spawn_search();
                }
            }
        }
        KeyCode::F(5) => {
            // Refresh interface list (rescan for new serial devices, etc.)
            app.discover_interfaces();
        }
        KeyCode::Esc => {
            app.current_screen = Screen::InterfaceTypeSelection;
        }
        _ => {}
    }
}

fn handle_device_list(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down => {
            let max_idx = app.discovered_devices.len().saturating_sub(1);
            let i = match app.device_list_state.selected() {
                Some(i) => {
                    if i >= max_idx {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            app.device_list_state.select(Some(i));
            // Clear refresh message when user interacts
            app.device_list_refresh_message = None;
        }
        KeyCode::Up => {
            let max_idx = app.discovered_devices.len().saturating_sub(1);
            let i = match app.device_list_state.selected() {
                Some(i) => {
                    if i == 0 {
                        max_idx
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            app.device_list_state.select(Some(i));
            // Clear refresh message when user interacts
            app.device_list_refresh_message = None;
        }
        KeyCode::Enter => {
            app.selected_device_index = app.device_list_state.selected();
            app.device_list_refresh_message = None;
            app.current_screen = Screen::CommandMenu;
        }
        KeyCode::F(5) => {
            // Refresh device list using async search
            app.is_refresh_search = true;
            app.current_screen = Screen::Searching;
            app.spawn_search();
        }
        KeyCode::Esc => {
            app.current_screen = Screen::InterfaceSelection;
        }
        _ => {}
    }
}

fn handle_command_menu(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down => {
            let i = match app.command_menu_state.selected() {
                Some(i) => {
                    if i >= 2 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            app.command_menu_state.select(Some(i));
        }
        KeyCode::Up => {
            let i = match app.command_menu_state.selected() {
                Some(i) => {
                    if i == 0 {
                        2
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            app.command_menu_state.select(Some(i));
        }
        KeyCode::Enter => {
            let selected = app.command_menu_state.selected().unwrap_or(0);
            app.selected_command = Some(match selected {
                0 => Command::Reset,
                1 => Command::Erase,
                2 => Command::Flash,
                _ => Command::Reset,
            });

            // If flash command, ask for hex file first
            if matches!(app.selected_command, Some(Command::Flash)) {
                app.current_screen = Screen::HexFileInput;
                app.hex_file_input_mode = true;
            } else {
                app.current_screen = Screen::Executing;
                app.execute_command();
            }
        }
        KeyCode::Esc => {
            app.current_screen = Screen::DeviceList;
        }
        _ => {}
    }
}

fn handle_hex_file_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up => {
            // Navigate to older history
            if !app.hex_file_history.is_empty() {
                let new_index = match app.hex_file_history_index {
                    None => 0,
                    Some(idx) => {
                        if idx + 1 < app.hex_file_history.len() {
                            idx + 1
                        } else {
                            idx
                        }
                    }
                };
                app.hex_file_history_index = Some(new_index);
                if let Some(path) = app.hex_file_history.get(new_index) {
                    app.hex_file_path = path.clone();
                }
            }
        }
        KeyCode::Down => {
            // Navigate to newer history
            match app.hex_file_history_index {
                None => {}
                Some(0) => {
                    // At most recent, clear to allow manual entry
                    app.hex_file_history_index = None;
                    app.hex_file_path.clear();
                }
                Some(idx) => {
                    let new_index = idx - 1;
                    app.hex_file_history_index = Some(new_index);
                    if let Some(path) = app.hex_file_history.get(new_index) {
                        app.hex_file_path = path.clone();
                    }
                }
            }
        }
        KeyCode::Enter => {
            if !app.hex_file_path.is_empty() {
                app.hex_file_input_mode = false;
                app.hex_file_history_index = None;
                app.current_screen = Screen::Executing;
                app.execute_command();
            }
        }
        KeyCode::Tab => {
            // Switch to file browser
            app.hex_file_input_mode = false;
            app.hex_file_history_index = None;
            app.enter_file_browser();
            app.current_screen = Screen::FileBrowser;
        }
        KeyCode::Char(c) => {
            // Clear history index when typing
            app.hex_file_history_index = None;
            app.hex_file_path.push(c);
        }
        KeyCode::Backspace => {
            // Clear history index when editing
            app.hex_file_history_index = None;
            app.hex_file_path.pop();
        }
        KeyCode::Esc => {
            app.hex_file_input_mode = false;
            app.hex_file_path.clear();
            app.hex_file_history_index = None;
            app.current_screen = Screen::CommandMenu;
        }
        _ => {}
    }
}

fn handle_file_browser(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down => {
            let max_idx = app.file_browser_entries.len().saturating_sub(1);
            let i = match app.file_browser_list_state.selected() {
                Some(i) => {
                    if i >= max_idx {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            app.file_browser_list_state.select(Some(i));
        }
        KeyCode::Up => {
            let max_idx = app.file_browser_entries.len().saturating_sub(1);
            let i = match app.file_browser_list_state.selected() {
                Some(i) => {
                    if i == 0 {
                        max_idx
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            app.file_browser_list_state.select(Some(i));
        }
        KeyCode::Enter => {
            if let Some(idx) = app.file_browser_list_state.selected() {
                if let Some(entry) = app.file_browser_entries.get(idx).cloned() {
                    if entry.is_dir {
                        // Navigate into directory
                        app.file_browser_current_dir = entry.path;
                        app.populate_file_browser();
                    } else {
                        // Select file
                        app.hex_file_path = entry.path.to_string_lossy().to_string();
                        app.current_screen = Screen::Executing;
                        app.execute_command();
                    }
                }
            }
        }
        KeyCode::Esc => {
            app.current_screen = Screen::HexFileInput;
            app.hex_file_input_mode = true;
        }
        _ => {}
    }
}

fn handle_results(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Enter | KeyCode::Esc => {
            app.result_message.clear();
            app.error_message = None;
            app.hex_file_path.clear();
            app.current_screen = Screen::DeviceList;
        }
        _ => {}
    }
}

// ================================================================================================
// UI Rendering Functions
// ================================================================================================
//
// These functions use ratatui widgets to draw the various screens.
// Each screen is composed of layouts, lists, paragraphs, and styled text.
//
// Color scheme:
// - Cyan: Titles and headers
// - Blue: Selected/highlighted items
// - Green: Success messages, active elements
// - Yellow: Progress indicators, warnings
// - Red: Error messages
// - Gray: Help text
// ================================================================================================

/// Main UI dispatcher - renders the appropriate screen based on app state.
fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    match app.current_screen {
        Screen::InterfaceTypeSelection => draw_interface_type_selection(f, app, size),
        Screen::InterfaceSelection => draw_interface_selection(f, app, size),
        Screen::Searching => {
            // Draw device list in background with overlay
            draw_device_list(f, app, size);
            draw_search_overlay(f, app);
        }
        Screen::DeviceList => draw_device_list(f, app, size),
        Screen::CommandMenu => draw_command_menu(f, app, size),
        Screen::HexFileInput => draw_hex_file_input(f, app, size),
        Screen::FileBrowser => draw_file_browser(f, app, size),
        Screen::Executing => draw_executing(f, app, size),
        Screen::Results => draw_results(f, app, size),
    }
}

/// Create a centered rect for popup
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Draw a search overlay popup
fn draw_search_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 30, f.area());

    // Clear the background
    f.render_widget(Clear, area);

    let interface_name = app.selected_interface.as_deref().unwrap_or("Unknown");
    let interface_type = app
        .selected_interface_type
        .as_ref()
        .map(|it| it.as_str())
        .unwrap_or("Unknown");

    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "⏳ Searching for devices...",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Interface: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{} ({})", interface_name, interface_type)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Please wait, do not interact with the TUI",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )]),
        Line::from(""),
    ];

    let block = Block::default()
        .title(" Device Search ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_interface_type_selection(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("Frankly Firmware Update - Terminal UI")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = vec![
        ListItem::new("SIM (Simulated Device)"),
        ListItem::new("Serial (UART/USB)"),
        ListItem::new("CAN Bus"),
    ];

    let list = List::new(items)
        .block(
            Block::default()
                .title("Select Interface Type")
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = app.interface_type_state.clone();
    f.render_stateful_widget(list, chunks[1], &mut state);

    let help = Paragraph::new("Use ↑↓ to navigate, Enter to select, 'q' to quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

fn draw_interface_selection(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let interface_type = app
        .selected_interface_type
        .as_ref()
        .map(|it| it.as_str())
        .unwrap_or("Unknown");
    let title = Paragraph::new(format!("Select {} Interface", interface_type))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = app
        .available_interfaces
        .iter()
        .map(|interface| ListItem::new(interface.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Available Interfaces")
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = app.interface_list_state.clone();
    f.render_stateful_widget(list, chunks[1], &mut state);

    let help = Paragraph::new(
        "↑↓ to navigate | Enter to select | F5 to refresh | Esc to go back | 'q' to quit",
    )
    .style(Style::default().fg(Color::Gray))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

fn draw_device_list(f: &mut Frame, app: &App, area: Rect) {
    // Adjust constraints based on whether we have a refresh message
    let constraints = if app.device_list_refresh_message.is_some() {
        vec![
            Constraint::Length(3),
            Constraint::Length(3), // Refresh message
            Constraint::Min(10),
            Constraint::Length(3),
        ]
    } else {
        vec![
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(constraints)
        .split(area);

    let title = Paragraph::new("Select Device")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let (list_chunk, help_chunk) = if let Some(ref refresh_msg) = app.device_list_refresh_message {
        // Show refresh message
        let refresh_info = Paragraph::new(refresh_msg.as_str())
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(refresh_info, chunks[1]);
        (2, 3)
    } else {
        (1, 2)
    };

    let items: Vec<ListItem> = app
        .discovered_devices
        .iter()
        .map(|device| ListItem::new(device.display_name.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!("Found {} Device(s)", app.discovered_devices.len()))
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = app.device_list_state.clone();
    f.render_stateful_widget(list, chunks[list_chunk], &mut state);

    let help = Paragraph::new(
        "↑↓ to navigate | Enter to select | F5 to refresh | Esc to go back | 'q' to quit",
    )
    .style(Style::default().fg(Color::Gray))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[help_chunk]);
}

fn draw_command_menu(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let device = app.get_selected_device();
    let device_name = device.map(|d| d.display_name.as_str()).unwrap_or("Unknown");

    let title_info = Paragraph::new(vec![
        Line::from(Span::styled(
            "Select Command",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Device: ", Style::default().fg(Color::Green)),
            Span::raw(device_name),
        ]),
    ])
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title_info, chunks[0]);

    let items: Vec<ListItem> = vec![
        ListItem::new("Reset Device"),
        ListItem::new("Erase Application"),
        ListItem::new("Flash Firmware"),
    ];

    let list = List::new(items)
        .block(Block::default().title("Commands").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = app.command_menu_state.clone();
    f.render_stateful_widget(list, chunks[1], &mut state);

    let help = Paragraph::new("Use ↑↓ to navigate, Enter to execute, Esc to go back, 'q' to quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

fn draw_hex_file_input(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("Enter Hex File Path")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Show input with history indicator
    let input_title = if !app.hex_file_history.is_empty() {
        let history_info = match app.hex_file_history_index {
            Some(idx) => format!(
                "Firmware Hex File Path [History {}/{}]",
                idx + 1,
                app.hex_file_history.len()
            ),
            None => format!(
                "Firmware Hex File Path [History: {} entries]",
                app.hex_file_history.len()
            ),
        };
        history_info
    } else {
        "Firmware Hex File Path".to_string()
    };

    let input = Paragraph::new(app.hex_file_path.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().title(input_title).borders(Borders::ALL));
    f.render_widget(input, chunks[1]);

    // Update help text to mention history navigation if available
    let help_text_content = if !app.hex_file_history.is_empty() {
        "Enter the full path to the firmware hex file.\nExample: data/example_app_g431rb.hex\n\nPress Tab to open the file browser.\nUse ↑↓ arrows to navigate through previously used firmware files."
    } else {
        "Enter the full path to the firmware hex file.\nExample: data/example_app_g431rb.hex\n\nOr press Tab to open the file browser."
    };

    let help_text = Paragraph::new(help_text_content)
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help_text, chunks[2]);

    let help = if !app.hex_file_history.is_empty() {
        Paragraph::new(
            "Type path | ↑↓ for history | Tab for browser | Enter to flash | Esc to cancel",
        )
    } else {
        Paragraph::new("Type path | Tab for browser | Enter to flash | Esc to cancel")
    };

    let help_final = help
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help_final, chunks[3]);
}

fn draw_file_browser(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("Browse for Hex File")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let current_path = app.file_browser_current_dir.to_string_lossy().to_string();
    let path_display = Paragraph::new(current_path)
        .style(Style::default().fg(Color::Green))
        .block(
            Block::default()
                .title("Current Directory")
                .borders(Borders::ALL),
        );
    f.render_widget(path_display, chunks[1]);

    let items: Vec<ListItem> = app
        .file_browser_entries
        .iter()
        .map(|entry| {
            let display = if entry.is_dir {
                format!("[DIR]  {}/", entry.name)
            } else {
                format!("[FILE] {}", entry.name)
            };
            let style = if entry.is_dir {
                Style::default().fg(Color::Blue)
            } else {
                Style::default().fg(Color::Green)
            };
            ListItem::new(display).style(style)
        })
        .collect();

    let list_title = if app.file_browser_entries.is_empty() {
        "No .hex files found in this directory"
    } else {
        "Files and Directories (only .hex files shown)"
    };

    let list = List::new(items)
        .block(Block::default().title(list_title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = app.file_browser_list_state.clone();
    f.render_stateful_widget(list, chunks[2], &mut state);

    let help =
        Paragraph::new("↑↓ to navigate | Enter to select/open | Esc to go back to manual entry")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}

fn draw_executing(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Length(3), Constraint::Min(10)])
        .split(area);

    let command_name = app
        .selected_command
        .as_ref()
        .map(|c| c.as_str())
        .unwrap_or("Unknown");
    let title = Paragraph::new(format!("Executing: {}", command_name))
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let device = app.get_selected_device();
    let device_name = device.map(|d| d.display_name.as_str()).unwrap_or("Unknown");

    let mut info_lines = vec![
        Line::from(vec![
            Span::styled("Device: ", Style::default().fg(Color::Green)),
            Span::raw(device_name),
        ]),
        Line::from(vec![
            Span::styled("Command: ", Style::default().fg(Color::Green)),
            Span::raw(command_name),
        ]),
        Line::from(""),
    ];

    // Show progress bar if available
    if let Some((current, total)) = app.operation_progress {
        let percentage = (current as f32 / total as f32 * 100.0) as u32;

        // Create simple text-based progress bar
        let bar_width = 40;
        let filled = (bar_width as f32 * current as f32 / total as f32) as usize;
        let empty = bar_width - filled;
        let bar = format!("[{}{}]", "=".repeat(filled), "-".repeat(empty));

        info_lines.push(Line::from(vec![
            Span::styled("Progress: ", Style::default().fg(Color::Cyan)),
            Span::styled(bar, Style::default().fg(Color::Green)),
        ]));
        info_lines.push(Line::from(vec![Span::raw(format!(
            "          {}/{} pages ({}%)",
            current, total, percentage
        ))]));
    }

    if !app.operation_status.is_empty() {
        info_lines.push(Line::from(""));
        info_lines.push(Line::from(Span::styled(
            &app.operation_status,
            Style::default().fg(Color::Yellow),
        )));
    } else {
        info_lines.push(Line::from(""));
        info_lines.push(Line::from(Span::styled(
            "Please wait...",
            Style::default().fg(Color::Yellow),
        )));
    }

    let info = Paragraph::new(info_lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(info, chunks[1]);
}

fn draw_results(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("Operation Results")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let mut result_lines = Vec::new();

    if let Some(error) = &app.error_message {
        // Replace special characters that might cause display issues
        let error_clean = error
            .replace('\t', " ")
            .replace('\r', "")
            .replace('\n', " ");
        result_lines.push(Line::from(vec![Span::styled(
            format!("ERROR: {}", error_clean),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]));
    } else if !app.result_message.is_empty() {
        for msg in &app.result_message {
            // Replace special characters that might cause display issues
            let msg_clean = msg.replace('\t', " ").replace('\r', "").replace('\n', " ");
            result_lines.push(Line::from(Span::styled(
                msg_clean,
                Style::default().fg(Color::Green),
            )));
        }
    } else {
        result_lines.push(Line::from(Span::styled(
            "No results available",
            Style::default().fg(Color::Yellow),
        )));
    }

    let results = Paragraph::new(result_lines)
        .block(Block::default().title("Results").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(results, chunks[1]);

    let help = Paragraph::new("Press Enter or Esc to return to device list")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}
