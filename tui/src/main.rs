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
    Error,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::fs;
use std::sync::{Arc, Mutex};

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
    result_message: Vec<String>,
    error_message: Option<String>,
    status_message: String,
}

impl App {
    fn new() -> App {
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
            result_message: Vec::new(),
            error_message: None,
            status_message: String::new(),
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

    fn search_devices(&mut self) {
        self.discovered_devices.clear();
        self.status_message = "Searching for devices...".to_string();

        let interface_type = match &self.selected_interface_type {
            Some(it) => it.clone(),
            None => return,
        };

        match interface_type {
            InterfaceType::Sim => {
                SIMInterface::config_nodes(SIM_NODE_LST.to_vec()).ok();
                self.search_devices_internal::<SIMInterface>();
            }
            InterfaceType::Serial => {
                self.search_devices_internal::<SerialInterface>();
            }
            InterfaceType::CAN => {
                self.search_devices_internal::<CANInterface>();
            }
        }

        self.status_message.clear();

        if !self.discovered_devices.is_empty() {
            self.device_list_state.select(Some(0));
        }
    }

    fn search_devices_internal<I: ComInterface>(&mut self) {
        let interface_name = match &self.selected_interface {
            Some(name) => name,
            None => return,
        };

        let conn_params = match self.selected_interface_type.as_ref().unwrap() {
            InterfaceType::Sim => ComConnParams::for_sim_device(),
            InterfaceType::Serial => ComConnParams::for_serial_conn(interface_name, 115200),
            InterfaceType::CAN => ComConnParams::for_can_conn(interface_name),
        };

        if I::is_network() {
            // Multi-device network interface (CAN, SIM)
            match I::create() {
                Ok(mut interface) => {
                    if let Err(e) = interface.open(&conn_params) {
                        self.error_message = Some(format!("Failed to open interface: {:?}", e));
                        return;
                    }
                    match interface.scan_network() {
                        Ok(node_lst) => {
                            for node in node_lst {
                                // No logger needed during device search
                                match self.connect_device::<I>(&conn_params, Some(node), None) {
                                    Ok(device) => {
                                        let device_info = format!("{}", device)
                                            .replace('\t', " ")
                                            .replace('\r', "")
                                            .replace('\n', " ");
                                        let display_name = format!("Node {:3} - {}", node, device_info);
                                        self.discovered_devices.push(DiscoveredDevice {
                                            node_id: Some(node),
                                            display_name,
                                            device_info,
                                        });
                                    }
                                    Err(e) => {
                                        self.discovered_devices.push(DiscoveredDevice {
                                            node_id: Some(node),
                                            display_name: format!("Node {:3} - Error: {:?}", node, e),
                                            device_info: String::new(),
                                        });
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            self.error_message = Some(format!("Network scan failed: {:?}", e));
                        }
                    }
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to create interface: {:?}", e));
                }
            }
        } else {
            // Single device interface (Serial)
            // No logger needed during device search
            match self.connect_device::<I>(&conn_params, None, None) {
                Ok(device) => {
                    let device_info = format!("{}", device)
                        .replace('\t', " ")
                        .replace('\r', "")
                        .replace('\n', " ");
                    self.discovered_devices.push(DiscoveredDevice {
                        node_id: None,
                        display_name: device_info.clone(),
                        device_info,
                    });
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to connect: {:?}", e));
                }
            }
        }

        if self.discovered_devices.is_empty() && self.error_message.is_none() {
            self.error_message = Some("No devices found".to_string());
        }
    }

    fn execute_command(&mut self) {
        self.result_message.clear();
        self.error_message = None;

        let command = match &self.selected_command {
            Some(cmd) => cmd.clone(),
            None => return,
        };

        let interface_type = match &self.selected_interface_type {
            Some(it) => it.clone(),
            None => return,
        };

        match interface_type {
            InterfaceType::Sim => {
                SIMInterface::config_nodes(SIM_NODE_LST.to_vec()).ok();
                match command {
                    Command::Reset => self.reset_device::<SIMInterface>(),
                    Command::Erase => self.erase_device::<SIMInterface>(),
                    Command::Flash => self.flash_device::<SIMInterface>(),
                }
            }
            InterfaceType::Serial => match command {
                Command::Reset => self.reset_device::<SerialInterface>(),
                Command::Erase => self.erase_device::<SerialInterface>(),
                Command::Flash => self.flash_device::<SerialInterface>(),
            },
            InterfaceType::CAN => match command {
                Command::Reset => self.reset_device::<CANInterface>(),
                Command::Erase => self.erase_device::<CANInterface>(),
                Command::Flash => self.flash_device::<CANInterface>(),
            },
        }
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

    fn connect_device<I: ComInterface>(
        &self,
        conn_params: &ComConnParams,
        node_id: Option<u8>,
        log_fn: Option<Box<dyn Fn(&str) + Send>>,
    ) -> Result<Device<I>, Error> {
        let mut interface = I::create()?;
        interface.open(conn_params)?;
        if let Some(node) = node_id {
            interface.set_mode(ComMode::Specific(node))?;
        }

        let mut device = Device::new_with_logger(interface, log_fn);
        device.init()?;

        Ok(device)
    }

    fn reset_device<I: ComInterface>(&mut self) {
        let device = match self.get_selected_device() {
            Some(d) => d,
            None => return,
        };

        // Create message capture for progress logging
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_clone = messages.clone();

        let conn_params = self.get_conn_params();
        let logger = Some(Box::new(move |msg: &str| {
            messages_clone.lock().unwrap().push(msg.to_string());
        }) as Box<dyn Fn(&str) + Send>);

        match self.connect_device::<I>(&conn_params, device.node_id, logger) {
            Ok(mut dev) => {
                let device_str = format!("{}", dev)
                    .replace('\t', " ")
                    .replace('\r', "")
                    .replace('\n', " ");
                self.result_message.push(format!("Device: {}", device_str));

                match dev.reset() {
                    Ok(_) => {
                        // Add captured progress messages
                        for msg in messages.lock().unwrap().iter() {
                            self.result_message.push(msg.clone());
                        }
                        self.result_message.push("Device reset successfully".to_string());
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Reset failed: {:?}", e));
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to connect: {:?}", e));
            }
        }
    }

    fn erase_device<I: ComInterface>(&mut self) {
        let device = match self.get_selected_device() {
            Some(d) => d,
            None => return,
        };

        // Create message capture for progress logging
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_clone = messages.clone();

        let conn_params = self.get_conn_params();
        let logger = Some(Box::new(move |msg: &str| {
            messages_clone.lock().unwrap().push(msg.to_string());
        }) as Box<dyn Fn(&str) + Send>);

        match self.connect_device::<I>(&conn_params, device.node_id, logger) {
            Ok(mut dev) => {
                let device_str = format!("{}", dev)
                    .replace('\t', " ")
                    .replace('\r', "")
                    .replace('\n', " ");
                self.result_message.push(format!("Device: {}", device_str));

                match dev.erase() {
                    Ok(_) => {
                        // Add captured progress messages
                        for msg in messages.lock().unwrap().iter() {
                            self.result_message.push(msg.clone());
                        }
                        self.result_message.push("Application erased successfully".to_string());
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Erase failed: {:?}", e));
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to connect: {:?}", e));
            }
        }
    }

    fn flash_device<I: ComInterface>(&mut self) {
        let device = match self.get_selected_device() {
            Some(d) => d,
            None => return,
        };

        if self.hex_file_path.is_empty() {
            self.error_message = Some("Hex file path required".to_string());
            return;
        }

        let hex_file = match HexFile::from_file(&self.hex_file_path) {
            Ok(hf) => hf,
            Err(e) => {
                self.error_message = Some(format!("Failed to load hex file: {:?}", e));
                return;
            }
        };

        // Create message capture for progress logging
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_clone = messages.clone();

        let conn_params = self.get_conn_params();
        let logger = Some(Box::new(move |msg: &str| {
            messages_clone.lock().unwrap().push(msg.to_string());
        }) as Box<dyn Fn(&str) + Send>);

        match self.connect_device::<I>(&conn_params, device.node_id, logger) {
            Ok(mut dev) => {
                let device_str = format!("{}", dev)
                    .replace('\t', " ")
                    .replace('\r', "")
                    .replace('\n', " ");
                self.result_message.push(format!("Device: {}", device_str));
                self.result_message.push("Flashing firmware...".to_string());

                match dev.flash(&hex_file) {
                    Ok(_) => {
                        // Add captured progress messages
                        for msg in messages.lock().unwrap().iter() {
                            self.result_message.push(msg.clone());
                        }
                        self.result_message.push("Firmware flashed successfully".to_string());
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Flash failed: {:?}", e));
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to connect: {:?}", e));
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
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match app.current_screen {
                    Screen::InterfaceTypeSelection => handle_interface_type_selection(app, key.code),
                    Screen::InterfaceSelection => handle_interface_selection(app, key.code),
                    Screen::Searching => {} // No input during search
                    Screen::DeviceList => handle_device_list(app, key.code),
                    Screen::CommandMenu => handle_command_menu(app, key.code),
                    Screen::HexFileInput => handle_hex_file_input(app, key.code),
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
                    app.search_devices();
                    if !app.discovered_devices.is_empty() {
                        app.current_screen = Screen::DeviceList;
                    } else if app.error_message.is_some() {
                        app.current_screen = Screen::Results;
                    } else {
                        app.current_screen = Screen::InterfaceSelection;
                    }
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
        }
        KeyCode::Up => {
            let max_idx = app.discovered_devices.len().saturating_sub(1);
            let i = match app.device_list_state.selected() {
                Some(i) => if i == 0 { max_idx } else { i - 1 },
                None => 0,
            };
            app.device_list_state.select(Some(i));
        }
        KeyCode::Enter => {
            app.selected_device_index = app.device_list_state.selected();
            app.current_screen = Screen::CommandMenu;
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
                app.current_screen = Screen::Results;
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
        KeyCode::Enter => {
            app.hex_file_input_mode = false;
            app.current_screen = Screen::Executing;
            app.execute_command();
            app.current_screen = Screen::Results;
        }
        KeyCode::Char(c) => {
            app.hex_file_path.push(c);
        }
        KeyCode::Backspace => {
            app.hex_file_path.pop();
        }
        KeyCode::Esc => {
            app.hex_file_input_mode = false;
            app.hex_file_path.clear();
            app.current_screen = Screen::CommandMenu;
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
        Screen::Searching => draw_searching(f, app, size),
        Screen::DeviceList => draw_device_list(f, app, size),
        Screen::CommandMenu => draw_command_menu(f, app, size),
        Screen::HexFileInput => draw_hex_file_input(f, app, size),
        Screen::Executing => draw_executing(f, app, size),
        Screen::Results => draw_results(f, app, size),
    }
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

fn draw_searching(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
        ])
        .split(area);

    let title = Paragraph::new("Searching for Devices...")
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let interface_name = app.selected_interface.as_ref().map(|s| s.as_str()).unwrap_or("Unknown");
    let info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Interface Type: ", Style::default().fg(Color::Green)),
            Span::raw(app.selected_interface_type.as_ref().map(|it| it.as_str()).unwrap_or("Unknown")),
        ]),
        Line::from(vec![
            Span::styled("Interface: ", Style::default().fg(Color::Green)),
            Span::raw(interface_name),
        ]),
        Line::from(""),
        Line::from(Span::styled("Scanning for devices...", Style::default().fg(Color::Yellow))),
    ])
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: true });
    f.render_widget(info, chunks[1]);
}

fn draw_device_list(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("Select Device")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

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
    f.render_stateful_widget(list, chunks[1], &mut state);

    let help = Paragraph::new("Use ↑↓ to navigate, Enter to select, Esc to go back, 'q' to quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
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

    let input = Paragraph::new(app.hex_file_path.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().title("Firmware Hex File Path").borders(Borders::ALL));
    f.render_widget(input, chunks[1]);

    let help_text = Paragraph::new("Enter the full path to the firmware hex file.\nExample: data/example_app_g431rb.hex")
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help_text, chunks[2]);

    let help = Paragraph::new("Type file path, Enter to flash, Esc to cancel")
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

    let info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Device: ", Style::default().fg(Color::Green)),
            Span::raw(device_name),
        ]),
        Line::from(vec![
            Span::styled("Command: ", Style::default().fg(Color::Green)),
            Span::raw(command_name),
        ]),
        Line::from(""),
        Line::from(Span::styled("Please wait...", Style::default().fg(Color::Yellow))),
    ])
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
