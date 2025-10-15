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
use std::io;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

const SIM_NODE_LST: [u8; 4] = [1, 3, 31, 8];

#[derive(Debug, Clone, PartialEq)]
enum InterfaceType {
    Sim,
    Serial,
    CAN,
}

impl InterfaceType {
    fn as_str(&self) -> &str {
        match self {
            InterfaceType::Sim => "SIM",
            InterfaceType::Serial => "Serial",
            InterfaceType::CAN => "CAN",
        }
    }
}

#[derive(Debug, Clone)]
struct DiscoveredDevice {
    node_id: Option<u8>,
    display_name: String,
    device_info: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Screen {
    InterfaceTypeSelection,
    InterfaceSelection,
    Searching,
    DeviceList,
    CommandMenu,
    HexFileInput,
    FileBrowser,
    Executing,
    Results,
}

#[derive(Debug, Clone, PartialEq)]
enum Command {
    Reset,
    Erase,
    Flash,
}

impl Command {
    fn as_str(&self) -> &str {
        match self {
            Command::Reset => "Reset Device",
            Command::Erase => "Erase Application",
            Command::Flash => "Flash Firmware",
        }
    }
}

#[derive(Debug)]
enum OperationMessage {
    Progress(ProgressUpdate),
    DeviceInfo(String),
    Complete,
    Error(String),
}

#[derive(Debug)]
enum SearchMessage {
    DeviceFound(DiscoveredDevice),
    Complete,
    Error(String),
}

#[derive(Debug, Clone)]
struct FileEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

struct App {
    current_screen: Screen,
    interface_type_state: ListState,
    selected_interface_type: Option<InterfaceType>,
    available_interfaces: Vec<String>,
    interface_list_state: ListState,
    selected_interface: Option<String>,
    discovered_devices: Vec<DiscoveredDevice>,
    device_list_state: ListState,
    selected_device_index: Option<usize>,
    command_menu_state: ListState,
    selected_command: Option<Command>,
    hex_file_path: String,
    hex_file_input_mode: bool,
    hex_file_history: Vec<String>,
    hex_file_history_index: Option<usize>,
    result_message: Vec<String>,
    error_message: Option<String>,
    device_list_refresh_message: Option<String>,
    // Progress tracking
    operation_progress: Option<(u32, u32)>, // (current, total)
    operation_status: String,
    operation_receiver: Option<Receiver<OperationMessage>>,
    // Search tracking
    search_receiver: Option<Receiver<SearchMessage>>,
    is_refresh_search: bool,
    // File browser
    file_browser_current_dir: PathBuf,
    file_browser_entries: Vec<FileEntry>,
    file_browser_list_state: ListState,
}

impl App {
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
        app.interface_type_state.select(Some(0));
        app.command_menu_state.select(Some(0));
        app
    }

    fn discover_interfaces(&mut self) {
        self.available_interfaces.clear();

        match self.selected_interface_type.as_ref().unwrap() {
            InterfaceType::Sim => {
                self.available_interfaces.push("sim".to_string());
            }
            InterfaceType::Serial => {
                // Enumerate serial ports
                match serialport::available_ports() {
                    Ok(ports) => {
                        for port in ports {
                            self.available_interfaces.push(port.port_name);
                        }
                    }
                    Err(_) => {
                        self.error_message = Some("Failed to enumerate serial ports".to_string());
                    }
                }

                if self.available_interfaces.is_empty() {
                    self.error_message = Some("No serial ports found".to_string());
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

        if !self.available_interfaces.is_empty() {
            self.interface_list_state.select(Some(0));
        }
    }

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
                    Command::Reset => self.spawn_operation::<SIMInterface>(tx, conn_params, device_node, None),
                    Command::Erase => self.spawn_erase::<SIMInterface>(tx, conn_params, device_node),
                    Command::Flash => self.spawn_flash::<SIMInterface>(tx, conn_params, device_node, hex_file_path),
                }
            }
            InterfaceType::Serial => match command {
                Command::Reset => self.spawn_operation::<SerialInterface>(tx, conn_params, device_node, None),
                Command::Erase => self.spawn_erase::<SerialInterface>(tx, conn_params, device_node),
                Command::Flash => self.spawn_flash::<SerialInterface>(tx, conn_params, device_node, hex_file_path),
            },
            InterfaceType::CAN => match command {
                Command::Reset => self.spawn_operation::<CANInterface>(tx, conn_params, device_node, None),
                Command::Erase => self.spawn_erase::<CANInterface>(tx, conn_params, device_node),
                Command::Flash => self.spawn_flash::<CANInterface>(tx, conn_params, device_node, hex_file_path),
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
                    tx.send(OperationMessage::Error(format!("Failed to create interface: {:?}", e))).ok();
                    return;
                }
            };

            if let Err(e) = interface.open(&conn_params) {
                tx.send(OperationMessage::Error(format!("Failed to open interface: {:?}", e))).ok();
                return;
            }

            if let Some(node) = node_id {
                if let Err(e) = interface.set_mode(ComMode::Specific(node)) {
                    tx.send(OperationMessage::Error(format!("Failed to set node mode: {:?}", e))).ok();
                    return;
                }
            }

            let mut device = Device::new_with_progress(interface, progress_fn);
            if let Err(e) = device.init() {
                tx.send(OperationMessage::Error(format!("Failed to initialize device: {:?}", e))).ok();
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
                    tx.send(OperationMessage::Error(format!("Reset failed: {:?}", e))).ok();
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
                    tx.send(OperationMessage::Error(format!("Failed to create interface: {:?}", e))).ok();
                    return;
                }
            };

            if let Err(e) = interface.open(&conn_params) {
                tx.send(OperationMessage::Error(format!("Failed to open interface: {:?}", e))).ok();
                return;
            }

            if let Some(node) = node_id {
                if let Err(e) = interface.set_mode(ComMode::Specific(node)) {
                    tx.send(OperationMessage::Error(format!("Failed to set node mode: {:?}", e))).ok();
                    return;
                }
            }

            let mut device = Device::new_with_progress(interface, progress_fn);
            if let Err(e) = device.init() {
                tx.send(OperationMessage::Error(format!("Failed to initialize device: {:?}", e))).ok();
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
                    tx.send(OperationMessage::Error(format!("Erase failed: {:?}", e))).ok();
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
                    tx.send(OperationMessage::Error(format!("Failed to load hex file: {:?}", e))).ok();
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
                    tx.send(OperationMessage::Error(format!("Failed to create interface: {:?}", e))).ok();
                    return;
                }
            };

            if let Err(e) = interface.open(&conn_params) {
                tx.send(OperationMessage::Error(format!("Failed to open interface: {:?}", e))).ok();
                return;
            }

            if let Some(node) = node_id {
                if let Err(e) = interface.set_mode(ComMode::Specific(node)) {
                    tx.send(OperationMessage::Error(format!("Failed to set node mode: {:?}", e))).ok();
                    return;
                }
            }

            let mut device = Device::new_with_progress(interface, progress_fn);
            if let Err(e) = device.init() {
                tx.send(OperationMessage::Error(format!("Failed to initialize device: {:?}", e))).ok();
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
                    tx.send(OperationMessage::Error(format!("Flash failed: {:?}", e))).ok();
                }
            }
        });
    }

    fn get_selected_device(&self) -> Option<&DiscoveredDevice> {
        self.selected_device_index
            .and_then(|idx| self.discovered_devices.get(idx))
    }

    fn get_conn_params(&self) -> ComConnParams {
        let interface_name = self.selected_interface.as_ref().unwrap();
        match self.selected_interface_type.as_ref().unwrap() {
            InterfaceType::Sim => ComConnParams::for_sim_device(),
            InterfaceType::Serial => ComConnParams::for_serial_conn(interface_name, 115200),
            InterfaceType::CAN => ComConnParams::for_can_conn(interface_name),
        }
    }

    fn process_operation_messages(&mut self) {
        let mut operation_complete = false;
        let mut operation_error = None;

        if let Some(ref receiver) = self.operation_receiver {
            // Non-blocking check for messages
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    OperationMessage::Progress(update) => {
                        match update {
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
                        }
                    }
                    OperationMessage::DeviceInfo(info) => {
                        self.result_message.push(format!("Device: {}", info));
                    }
                    OperationMessage::Complete => {
                        self.result_message.push("Operation completed successfully".to_string());
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

    fn populate_file_browser(&mut self) {
        self.file_browser_entries.clear();

        // Add parent directory entry if not at root
        if self.file_browser_current_dir.parent().is_some() {
            self.file_browser_entries.push(FileEntry {
                name: "..".to_string(),
                path: self.file_browser_current_dir.parent().unwrap().to_path_buf(),
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
        self.file_browser_current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        self.populate_file_browser();
    }

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
                        tx.send(SearchMessage::Error(format!("Failed to open interface: {:?}", e))).ok();
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
                                        })).ok();
                                    }
                                    Err(e) => {
                                        tx.send(SearchMessage::DeviceFound(DiscoveredDevice {
                                            node_id: Some(node),
                                            display_name: format!("Node {:3} - Error: {:?}", node, e),
                                            device_info: String::new(),
                                        })).ok();
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tx.send(SearchMessage::Error(format!("Network scan failed: {:?}", e))).ok();
                            return;
                        }
                    }
                }
                Err(e) => {
                    tx.send(SearchMessage::Error(format!("Failed to create interface: {:?}", e))).ok();
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
                    })).ok();
                }
                Err(e) => {
                    tx.send(SearchMessage::Error(format!("Failed to connect: {:?}", e))).ok();
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        // Process background operation messages
        app.process_operation_messages();
        // Process background search messages
        app.process_search_messages();

        terminal.draw(|f| ui(f, app))?;

        // Use poll with timeout for responsive UI updates
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.current_screen {
                        Screen::InterfaceTypeSelection => handle_interface_type_selection(app, key.code),
                        Screen::InterfaceSelection => handle_interface_selection(app, key.code),
                        Screen::Searching => {} // No input during search
                        Screen::DeviceList => handle_device_list(app, key.code),
                        Screen::CommandMenu => handle_command_menu(app, key.code),
                        Screen::HexFileInput => handle_hex_file_input(app, key.code),
                        Screen::FileBrowser => handle_file_browser(app, key.code),
                        Screen::Executing => {} // No input during execution
                        Screen::Results => handle_results(app, key.code),
                    }

                    // Global quit
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

fn handle_interface_type_selection(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down => {
            let i = match app.interface_type_state.selected() {
                Some(i) => if i >= 2 { 0 } else { i + 1 },
                None => 0,
            };
            app.interface_type_state.select(Some(i));
        }
        KeyCode::Up => {
            let i = match app.interface_type_state.selected() {
                Some(i) => if i == 0 { 2 } else { i - 1 },
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
                Some(i) => if i >= max_idx { 0 } else { i + 1 },
                None => 0,
            };
            app.interface_list_state.select(Some(i));
        }
        KeyCode::Up => {
            let max_idx = app.available_interfaces.len().saturating_sub(1);
            let i = match app.interface_list_state.selected() {
                Some(i) => if i == 0 { max_idx } else { i - 1 },
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
                Some(i) => if i >= max_idx { 0 } else { i + 1 },
                None => 0,
            };
            app.device_list_state.select(Some(i));
            // Clear refresh message when user interacts
            app.device_list_refresh_message = None;
        }
        KeyCode::Up => {
            let max_idx = app.discovered_devices.len().saturating_sub(1);
            let i = match app.device_list_state.selected() {
                Some(i) => if i == 0 { max_idx } else { i - 1 },
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
                Some(i) => if i >= 2 { 0 } else { i + 1 },
                None => 0,
            };
            app.command_menu_state.select(Some(i));
        }
        KeyCode::Up => {
            let i = match app.command_menu_state.selected() {
                Some(i) => if i == 0 { 2 } else { i - 1 },
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
                Some(i) => if i >= max_idx { 0 } else { i + 1 },
                None => 0,
            };
            app.file_browser_list_state.select(Some(i));
        }
        KeyCode::Up => {
            let max_idx = app.file_browser_entries.len().saturating_sub(1);
            let i = match app.file_browser_list_state.selected() {
                Some(i) => if i == 0 { max_idx } else { i - 1 },
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

    let interface_name = app.selected_interface.as_ref().map(|s| s.as_str()).unwrap_or("Unknown");
    let interface_type = app.selected_interface_type.as_ref().map(|it| it.as_str()).unwrap_or("Unknown");

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("⏳ Searching for devices...", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Interface: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{} ({})", interface_name, interface_type)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Please wait, do not interact with the TUI", Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)),
        ]),
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
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = vec![
        ListItem::new("SIM (Simulated Device)"),
        ListItem::new("Serial (UART/USB)"),
        ListItem::new("CAN Bus"),
    ];

    let list = List::new(items)
        .block(Block::default().title("Select Interface Type").borders(Borders::ALL))
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

    let interface_type = app.selected_interface_type.as_ref().map(|it| it.as_str()).unwrap_or("Unknown");
    let title = Paragraph::new(format!("Select {} Interface", interface_type))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = app.available_interfaces
        .iter()
        .map(|interface| ListItem::new(interface.as_str()))
        .collect();

    let list = List::new(items)
        .block(Block::default().title("Available Interfaces").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = app.interface_list_state.clone();
    f.render_stateful_widget(list, chunks[1], &mut state);

    let help = Paragraph::new("Use ↑↓ to navigate, Enter to select and search, Esc to go back, 'q' to quit")
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
            Constraint::Length(3),  // Refresh message
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
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let (list_chunk, help_chunk) = if let Some(ref refresh_msg) = app.device_list_refresh_message {
        // Show refresh message
        let refresh_info = Paragraph::new(refresh_msg.as_str())
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(refresh_info, chunks[1]);
        (2, 3)
    } else {
        (1, 2)
    };

    let items: Vec<ListItem> = app.discovered_devices
        .iter()
        .map(|device| ListItem::new(device.display_name.as_str()))
        .collect();

    let list = List::new(items)
        .block(Block::default().title(format!("Found {} Device(s)", app.discovered_devices.len())).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = app.device_list_state.clone();
    f.render_stateful_widget(list, chunks[list_chunk], &mut state);

    let help = Paragraph::new("↑↓ to navigate | Enter to select | F5 to refresh | Esc to go back | 'q' to quit")
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
        Line::from(Span::styled("Select Command", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
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
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Show input with history indicator
    let input_title = if !app.hex_file_history.is_empty() {
        let history_info = match app.hex_file_history_index {
            Some(idx) => format!("Firmware Hex File Path [History {}/{}]", idx + 1, app.hex_file_history.len()),
            None => format!("Firmware Hex File Path [History: {} entries]", app.hex_file_history.len()),
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
        Paragraph::new("Type path | ↑↓ for history | Tab for browser | Enter to flash | Esc to cancel")
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
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let current_path = app.file_browser_current_dir.to_string_lossy().to_string();
    let path_display = Paragraph::new(current_path)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().title("Current Directory").borders(Borders::ALL));
    f.render_widget(path_display, chunks[1]);

    let items: Vec<ListItem> = app.file_browser_entries
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

    let help = Paragraph::new("↑↓ to navigate | Enter to select/open | Esc to go back to manual entry")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}

fn draw_executing(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
        ])
        .split(area);

    let command_name = app.selected_command.as_ref().map(|c| c.as_str()).unwrap_or("Unknown");
    let title = Paragraph::new(format!("Executing: {}", command_name))
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
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
        info_lines.push(Line::from(vec![
            Span::raw(format!("          {}/{} pages ({}%)", current, total, percentage)),
        ]));
    }

    if !app.operation_status.is_empty() {
        info_lines.push(Line::from(""));
        info_lines.push(Line::from(Span::styled(
            &app.operation_status,
            Style::default().fg(Color::Yellow),
        )));
    } else {
        info_lines.push(Line::from(""));
        info_lines.push(Line::from(Span::styled("Please wait...", Style::default().fg(Color::Yellow))));
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
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
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
            let msg_clean = msg
                .replace('\t', " ")
                .replace('\r', "")
                .replace('\n', " ");
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
