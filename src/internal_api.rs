use crate::PathBuf;
use rocket::serde::{Serialize};
use serde::Deserialize;
use enumset::{EnumSetType, EnumSet};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfo {
    pub last_modified_since_epoch: std::time::Duration,
    pub path: PathBuf,
    pub size: u64
}

#[derive(PartialEq)]
#[derive(Debug)]
#[derive(Copy, Clone)]
#[derive(Serialize, Deserialize)]
pub enum ProbePoint {
    HOTEND,
    BED,
    CHAMBER,
    PROBE,
    COOLER,
    REDUNDANT,
    UNKNOWN
}


#[derive(EnumSetType, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
    E
}

#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Default)]
#[derive(Copy, Clone)]
#[derive(Serialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub e: f64
}

#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Copy, Clone)]
#[derive(Serialize)]
pub struct Temperature {
    pub measured_from: ProbePoint,
    pub index: u32, // For multiple extruders which report T0, T1, etc...
    pub power: f64, // 0-1 percentage
    pub current: f64,
    pub target: f64
}

#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Copy, Clone, Deserialize)]
pub struct TemperatureTarget {
    pub to_set: ProbePoint,
    pub index: Option<u32>,
    pub target: f64
}


#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Copy, Clone)]
#[derive(Serialize)]
pub enum PrintState {
    CONNECTED,
    STARTED,
    PAUSED,
    DONE,
    DEAD
}

#[derive(Serialize, Debug, Clone)]
pub struct PrinterStatus {
    pub printer_connected: bool,
    pub manual_control_enabled: bool,
    pub state: PrintState,
    pub temperatures: Vec<Temperature>,
    pub position: Position,
    pub gcode_lines_done_total: Option<(String, u32, u32)>,
    pub print_time_remaining: Option<std::time::Duration>,
    pub fan_speed: Vec<f64>
}

impl Default for PrinterStatus {
    fn default() -> PrinterStatus {
        PrinterStatus { printer_connected: false, manual_control_enabled: false,state: PrintState::DEAD, temperatures: Vec::new(), gcode_lines_done_total: None, position: Position::default(), print_time_remaining: None,
        fan_speed: Vec::new() }
    }
}


pub enum PrinterCommand {
    Connect(PathBuf, u32),
    Disconnect,
    SetGcodeFile(PathBuf),
    DeleteGcodeFile(PathBuf),
    StartPrint,
    PausePrint,
    StopPrint,
    GetStatus,
    ManualMove(Position),
    Home(EnumSet<Axis>),
    SetTemperature(TemperatureTarget),
    SetFanSpeed((u32, f64))
}

pub enum PrinterResponse {
    GenericResult(std::io::Result<()>),
    Status(std::io::Result<PrinterStatus>),
}